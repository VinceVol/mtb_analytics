# mtb_analytics
Unfortunately the current free tools for understanding what I need to improve on in the MTB world are limited.
I plan on using this alongside tools like golden cheetah to understand why I suck.

### Fairy-tale land dreams
We all have dreams, here are mine for this "*software*"
- Upload garmin fit files by dragging them into Data folder
- Choose which segment you're interested in in the terminal options
- Run either an individualized analysis or a full one that shows where time is lost
--Show lost time video clip side by side with the PR/REF

## Active Todos!
- [ ] Update the gates for a segment bin to include every possible gate and then let the compare function be fed the frequency at which we want to read the gates
- [ ] Refactor the gate crossing logic to interpolate between gates to get time when a gate is missed
- [ ] Add a graph for simply looking at overall time at each gate and not just the splits
- [ ] Filter extraneous values in the initial segment gate generation
- [ ] Find a way to backtrack if a bunch of gates are being missed in a row -- should at least be able to determine the failure point, skip that and then restart. Should never end up with half the gates missed
