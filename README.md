# skeinrs

[skein in one page](https://github.com/leithaus/skeinrs/blob/main/SkeinInOnePage.jpg)

```
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

For the Apple Vision Pro

```
xed LeapSpigotVision
```

Then, if you have an AVP, build with vision_pro as the destination. Otherwise, build with simulator as the destination.