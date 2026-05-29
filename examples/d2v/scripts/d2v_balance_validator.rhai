// P1 Soll/Haben-Balance-Validator.
//
// Erwartet in `fields` (siehe ScriptInputs.fields):
//   value             — Betrag der aktuellen Buchungszeile (Number)
//   valueType         — "SOLL" oder "HABEN" (String, post-Wire-Decode)
//   partnerValue      — Betrag der Gegen-Zeile (Number)
//   partnerValueType  — "SOLL" oder "HABEN" der Gegen-Zeile (String)
//
// Liefert `true` wenn (SOLL-Summe == HABEN-Summe), sonst `false`.
// Konvention: Spec §2.2#4 — read-time Plausibilitaet, keine Exception.
let v  = fields.value;
let vt = fields.valueType;
let pv = fields.partnerValue;
let pt = fields.partnerValueType;

let soll  = if vt == "SOLL"  { v } else { 0 };
let soll2 = if pt == "SOLL"  { pv } else { 0 };
let haben = if vt == "HABEN" { v } else { 0 };
let haben2= if pt == "HABEN" { pv } else { 0 };

(soll + soll2) == (haben + haben2)
