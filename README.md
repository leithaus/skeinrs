# skeinrs

[skein in one page](https://github.com/leithaus/skeinrs/blob/main/SkeinInOnePage.jpg)

```
cd leap_spigot
cargo build
```

Or, if you have a LeapMotion controller set up, then build with

```
cargo build --features leap
```

Then to run the application:

```
cargo run -- --quick
```
```--quick``` 
skips the interactive configuration and launches immediately with the defaults (π for durations, e for pitches, C major, piano, 120 BPM).
Or without 
```--quick```
to configure everything interactively first:
bashcargo run
It will ask you to pick constants, bases, scale, instrument, tempo, then open the visualizer window.

