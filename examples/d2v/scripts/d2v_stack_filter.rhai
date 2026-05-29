// P3 DatevEntryStack-Filter.
//
// Erwartet in `fields`:
//   stackId          — der stackId-Wert der Zeile (Integer)
//   selectedStackId  — der vom Filter-UI gewaehlte Stack (Integer; -1
//                      bzw. fehlend = "alle Stacks", → true)
//
// Liefert `true`, wenn die Zeile zum gewaehlten Stack gehoert (oder kein
// Stack ausgewaehlt ist).
let sel = fields.selectedStackId;
let row = fields.stackId;
if sel == () || sel == -1 { true } else { row == sel }
