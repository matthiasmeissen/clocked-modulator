# Enclosure

- [`case.step`](case.step): the lower part that holds the PCB
- [`cover.step`](cover.step): the top part
- [`encoder_caps.step`](encoder_caps.step): the two encoder knobs, for a 6 mm flatted shaft

The other parts are listed in the [BOM](../hardware/fab/bom.csv).

Import the STEP files into your slicer to print them, or into a CAD program to edit them.

The case takes threaded inserts. 1 cm spacers thread into them and clamp the
PCB; the cover lies on the spacers and is screwed into them. The STEP files
were made for CNC milling, so enlarge the bottom holes of the case to fit your
inserts before printing.

<!-- TODO: confirm the screw and insert size (probably M2.5), then add inserts, spacers and screws to the BOM -->
