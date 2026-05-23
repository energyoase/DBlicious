## Anwendung
app-title = DBlicious
app-loading = Wird geladen…
app-error = Fehler: { $message }

## Navigation
nav-dashboard = Übersicht
nav-catalog = Katalog
nav-products = Produkte
nav-categories = Kategorien
nav-categories-active = Aktive Kategorien
nav-categories-archived = Archivierte Kategorien
nav-categories-archived-2024 = Archiv 2024
nav-sales = Verkauf
nav-orders = Bestellungen
nav-customers = Kunden
nav-designer = Schema-Designer
nav-builder = ⚙ Layout

## Tabelle
table-empty = Keine Datensätze vorhanden.
table-loading = Tabelle wird geladen…
table-placeholder-complex = ⟨komplexer Wert⟩
table-placeholder-reference = ⟨Verweis⟩
table-placeholder-collection = ⟨{ $count ->
    [one] 1 Eintrag
   *[other] { $count } Einträge
}⟩
table-actions-sort = Sortieren
table-actions-filter = Filtern
table-actions-search = Suchen
table-pagination-summary = Seite { $page } von { $total }
table-pagination-range = { $from }–{ $to } von { $count }
table-pagination-prev = Zurück
table-pagination-next = Weiter

## Felder (Spaltentitel)
field-id = Kennung
field-name = Name
field-price = Preis
field-in_stock = Auf Lager
field-created_at = Angelegt am
field-category = Kategorie
field-tags = Schlagwörter
field-order_number = Bestellnummer
field-total = Gesamtbetrag
field-placed_at = Bestellt am
field-status = Status
field-customer = Kunde
field-display_name = Anzeigename
field-email = E-Mail
field-member_since = Mitglied seit
field-order_count = Bestellungen

## Werte
value-bool-true = Ja
value-bool-false = Nein

## Sprachumschalter
locale-de = Deutsch
locale-en = English
locale-fr = Français

## Validation
validation-required = Dieses Feld darf nicht leer sein.
validation-min_length = Mindestens { $min } Zeichen erforderlich.
validation-max_length = Höchstens { $max } Zeichen erlaubt.
validation-number_range = Wert muss zwischen { $min } und { $max } liegen.
validation-pattern = Wert entspricht nicht dem erwarteten Muster.
validation-enum_value = Wert ist in dieser Auswahl nicht zulässig.

## Editor
editor-title-new = Neuer Datensatz
editor-title-edit = { $type } bearbeiten
editor-section-master = Stammdaten
editor-placeholder-complex = ⟨nicht editierbar⟩
editor-actions-save = Speichern
editor-actions-saving = Speichern …
editor-actions-cancel = Abbrechen
editor-actions-delete = Löschen
editor-actions-back = Zurück
editor-state-dirty = Nicht gespeicherte Änderungen
editor-state-saved = Gespeichert
editor-confirm-delete = Diesen Datensatz wirklich löschen?

## Error
error-decode = Daten konnten nicht verarbeitet werden.
error-invalid_identifier = Ungültiger Bezeichner: { $id }
error-network = Netzwerkfehler.
error-validation = Eingaben unvollständig oder ungültig.
error-concurrent_modification = Datensatz wurde zwischenzeitlich geändert. Bitte neu laden.
error-other = Unerwarteter Fehler.

## Security / Login
security-group-admin = Administratoren
security-group-admin-desc = Volle Rechte auf alle Entitäten.
security-group-editor = Redaktion
security-group-editor-desc = Darf Daten erstellen und ändern, aber nicht löschen.
security-group-viewer = Leser
security-group-viewer-desc = Reiner Lesezugriff.

login-title = Anmelden
login-username = Benutzername
login-password = Passwort
login-submit = Anmelden
login-error-invalidCredentials = Benutzername oder Passwort falsch.
login-error-inactive = Konto ist deaktiviert.
login-error-internal = Interner Fehler.
login-hint = Probier admin/admin, editor/editor oder viewer/viewer.

## Topbar
topbar-logout = Abmelden
topbar-user = { $name }

## Tabelle (zusätzlich)
table-actions-new = Neu
table-actions-edit = Bearbeiten
table-actions-delete = Löschen
table-actions-builder = Layout bearbeiten

## Designer
designer-title = Schema-Designer
designer-forbidden = Du hast keine Berechtigung, das Schema zu bearbeiten.
designer-hint = Tabellen per Drag verschieben. Im Verknüpfungsmodus zwei Spalten-Ports anklicken, um eine Beziehung anzulegen. Klick auf eine Linie löscht sie.
designer-fields-schema_name = Schemaname
designer-actions-add_table = Tabelle hinzufügen
designer-actions-remove_table = Tabelle entfernen
designer-actions-add_column = Spalte hinzufügen
designer-actions-remove_column = Spalte entfernen
designer-actions-save = Schema speichern
designer-actions-saving = Wird gespeichert…
designer-actions-link_mode_off = Verknüpfungsmodus aus
designer-actions-link_mode_on = Verknüpfungsmodus an
designer-column-add_hint = Spalten
designer-column-toggle_pk = Primärschlüssel umschalten
designer-port-tooltip = Im Verknüpfungsmodus anklicken, um eine Beziehung anzulegen
designer-relation-tooltip = Klicken, um die Beziehung zu entfernen

## Builder (Visual UI-Designer)
builder-title = Visual Builder
builder-subtitle = Entity: { $entity }
builder-forbidden = Du hast keine Berechtigung, den Builder zu nutzen.
builder-preview-title = Live-Vorschau
builder-action-add = Knoten hinzufügen
builder-action-delete = Knoten löschen
builder-action-undo = Rückgängig
builder-action-redo = Wiederholen
builder-action-save = Speichern
builder-action-reload = Server-Stand laden
builder-nodes_count = { $n } Knoten
builder-status-idle = Nicht gespeichert
builder-status-loading = Wird geladen…
builder-status-saving = Wird gespeichert…
builder-status-saved = Gespeichert (Version { $version })
builder-status-conflict = Konflikt — Server hat Version { $version }
builder-status-error = Fehler: { $message }

## Column-Editor (Q0005)
column-editor-title        = Spalte „{ $name }"
column-editor-visibility   = Sichtbar
column-editor-position     = Position
column-editor-min-width    = Min-Breite
column-editor-label        = Label
column-editor-sortable     = Sortierbar
column-editor-filter       = Filter
column-editor-format       = Format
column-editor-reset        = Zurücksetzen
column-editor-preview      = Vorschau

table-actions-edit-mode    = Layout bearbeiten
table-actions-save-view    = Speichern
table-actions-discard-view = Verwerfen
table-status-edit-layer    = Layer: { $layer }
table-status-pending       = { $n } ungespeicherte Änderungen
table-fallback-view        = Ansicht „{ $name }" nicht gefunden — zeige Default

## Filter-Labels
filter-contains      = Enthält
filter-equals        = Gleich
filter-range         = Bereich
filter-text-contains = Enthält (Text)
filter-number-range  = Bereich (Zahl)
filter-bool-equals   = Gleich (Ja/Nein)
filter-enum-in       = Auswahl
filter-date-range    = Datumsbereich

## Konflikt-Meldung (Named Views)
table-view-conflict = Konflikt: andere Bearbeitung wurde zwischenzeitlich gespeichert. Bitte neu laden und Edits nochmal anwenden.

## Formatter-Labels
formatter-money-symbol    = EUR-Symbol €
formatter-money-code      = EUR-Code
formatter-money-decimals  = Nur Dezimalen
formatter-decimal-default = Standard
formatter-decimal-2       = 2 Nachkommastellen
formatter-date-iso        = ISO (YYYY-MM-DD)
formatter-date-local      = Lokal
formatter-datetime-iso    = ISO Datum+Zeit
formatter-datetime-local  = Lokal Datum+Zeit
formatter-int-default     = Standard
formatter-bool-yesno      = Ja / Nein
formatter-text-plain      = Klartext
