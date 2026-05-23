// Fixture-Template fuer 1.7.9 Typst-PDF-Backend.
// Vars kommen als JSON-String unter sys.inputs.data (siehe
// server/src/pdf/typst.rs) — hier geparst und gerendert.
#import sys: inputs
#let data = json(bytes(inputs.data))

#set page(width: 210mm, height: 297mm, margin: 20mm)
#set text(size: 11pt)

= Rechnung #data.at("invoice_no", default: "—")

Kunde: #data.at("customer_name", default: "—")

Betrag: #data.at("total", default: "0") EUR
