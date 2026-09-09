use nalgebra::{Point2, RealField, Vector2};
use rkyv::{Archive, Deserialize, Serialize};
use utm::{lat_lon_to_zone_number, lat_to_zone_letter, to_utm_wgs84_no_zone, wsg84_utm_to_lat_lon};
// using gate as a method of capturing when the rider goes through a part on the path.
// a line from left to right pivot show the gate itself and the in/out ref are to be able
// to compare whether the rider is going through the gate in the right direction
// ---------------
// for now I dont think I actually need a inside and outside ref given that
// segments are run in one direction and that gates are crossed in order
#[derive(Debug, Archive, Serialize, Deserialize, Clone, Copy)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Gate {
    pub left_pivot: (f32, f32),  //lon, lat
    pub right_pivot: (f32, f32), //lon, lat
}

impl Gate {
    //length is in meters
    pub fn new(points: [(f32, f32); 3], length_m: f64) -> Gate {
        let (utm_points, ref_mat) = points_to_utm(points);
        //check the data_analytics image under gates (2.1) for what the goal is here
        let point_a = Point2::new(utm_points[0].0, utm_points[0].1);
        let point_b = Point2::new(utm_points[1].0, utm_points[1].1);
        let point_c = Point2::new(utm_points[2].0, utm_points[2].1);

        //vectorize the two lines then grab the normalized perpindicular line
        let vec_ab: Vector2<f64> = point_b - point_a;
        let vec_bc: Vector2<f64> = point_c - point_b;
        let vec_ab_perp: Vector2<f64> = Vector2::new(-vec_ab.y, vec_ab.x);
        let vec_bc_perp: Vector2<f64> = Vector2::new(-vec_bc.y, vec_bc.x);
        let norm_vec_ab_perp = vec_ab_perp.normalize() * length_m / 2.0;
        let norm_vec_bc_perp = vec_bc_perp.normalize() * length_m / 2.0;

        //Form those two gates based on the two vectors than take the midpoints
        // to form the new finalized gate
        let ab_gate = (point_a - norm_vec_ab_perp, point_b + norm_vec_ab_perp);
        let bc_gate = (point_b - norm_vec_bc_perp, point_c + norm_vec_bc_perp);
        let full_gate: Vector2<f64> = (ab_gate.1 + (bc_gate.1 - ab_gate.1) / 2.0)
            - (ab_gate.0 + (bc_gate.0 - ab_gate.0) / 2.0);

        //Normalize the full gate and apply the length to it
        let full_norm = full_gate.normalize() * length_m / 2.0;

        let full_left = point_b - full_norm;
        let full_right = point_b + full_norm;

        let left_point = (full_left.x, full_left.y);
        let right_point = (full_right.x, full_right.y);

        //They're both using the same zone and letter because they should be very close to one another
        Gate {
            left_pivot: utm_to_points([left_point], [ref_mat[0]])[0],
            right_pivot: utm_to_points([right_point], [ref_mat[0]])[0],
        }
    }
    pub fn is_crossed(&self, points: [(f32, f32); 2]) -> bool {
        //Convert the GPS stuff into math applicable format
        let (utm_points, _) = points_to_utm(points);
        let point_a = Point2::new(utm_points[0].0, utm_points[0].1);
        let point_b = Point2::new(utm_points[1].0, utm_points[1].1);

        let (utm_points_gate, _) = points_to_utm([self.left_pivot, self.right_pivot]);
        let point_c = Point2::new(utm_points_gate[0].0, utm_points_gate[0].1);
        let point_d = Point2::new(utm_points_gate[1].0, utm_points_gate[1].1);

        //ab is the activity vector cd is the gate we're checking for an intersection with
        let vec_ab = point_b - point_a;
        let vec_cd = point_d - point_c;

        #[inline]
        fn cross_2d(a: &Vector2<f64>, b: &Vector2<f64>) -> f64 {
            a.x * b.y - a.y * b.x
        }

        // println!(
        //     "Points [a,b,c,d] = [{},{},{},{}]",
        //     point_a, point_b, point_c, point_d
        // );
        // println!("Vector [ab,cd] = [{},{}]", vec_ab, vec_cd);
        // println!("cross_2d = {}", cross_2d(&vec_ab, &vec_cd));
        //if the 2d cross product is 0 then the vectors are parallel (no intersecting the way we want)
        if cross_2d(&vec_ab, &vec_cd) == 0.0 {
            return false;
        }

        let t = cross_2d(&(point_c - point_a), &vec_cd) / cross_2d(&vec_ab, &vec_cd);
        let u = cross_2d(&(point_c - point_a), &vec_ab) / cross_2d(&vec_ab, &vec_cd);

        t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
    }
}
//Cannot do vector math directily on lon,lat points as the earth isn't flat
// utm should be a unit that works well with vectors
pub fn points_to_utm<const N: usize>(
    points: [(f32, f32); N],
) -> ([(f64, f64); N], [(u8, char); N]) {
    let mut utm_points: [(f64, f64); N] = [(0.0, 0.0); N];
    let mut zone_num_letter: [(u8, char); N] = [(0, 'a'); N];
    for (i, (lon, lat)) in points.iter().enumerate() {
        let (northing, easting, _) = to_utm_wgs84_no_zone(*lat as f64, *lon as f64);
        let zone = lat_lon_to_zone_number(*lat as f64, *lon as f64);
        let letter = lat_to_zone_letter(*lat as f64).unwrap_or('N');

        zone_num_letter[i] = (zone, letter);
        utm_points[i] = (easting, northing);
    }

    (utm_points, zone_num_letter)
}

pub fn utm_to_points<const N: usize>(
    utm_points: [(f64, f64); N],
    zone_num_letter: [(u8, char); N],
) -> [(f32, f32); N] {
    let mut points: [(f32, f32); N] = [(0.0, 0.0); N];
    for (i, (easting, northing)) in utm_points.iter().enumerate() {
        let (lat, lon) = wsg84_utm_to_lat_lon(
            *easting,
            *northing,
            zone_num_letter[i].0,
            zone_num_letter[i].1,
        )
        .unwrap();

        points[i] = (lon as f32, lat as f32);
    }

    points
}

pub fn dist_btwn_points<F>(point1: (F, F), point2: (F, F)) -> F
where
    F: RealField + Copy,
{
    //get into n_algebra structs
    let point_1 = Point2::new(point1.0, point1.1);
    let point_2 = Point2::new(point2.0, point2.1);
    let point_vec: Vector2<F> = point_2 - point_1;

    let dot = point_vec.dot(&point_vec);

    dot.sqrt()
}
