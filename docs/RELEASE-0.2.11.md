# Palisade Windows 0.2.11

- Exclude the entire desktop button area from the root window drag region, fixing controls that could appear frozen after the header spacing change.
- Explicitly mark the local controls and settings surface as non-draggable.
- Add a rendered-layout regression check that fails when the root drag region extends under the controls or hosted page.

Physical Windows mouse hit testing remains an operator check. The checks inspect actual rendered geometry and exercise button actions; they do not establish native mouse routing.
