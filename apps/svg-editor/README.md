# svg-editor (CLI)

What the profile promises, testable without a screen:

```
svg-editor check drawing.svg          # hold it to the profile; exit 1 on a finding
svg-editor shapes drawing.svg         # list its shapes in paint order
svg-editor render drawing.svg -o out.png [--scale 2]   # through resvg
```
