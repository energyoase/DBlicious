## Application
app-title = DBlicious
app-loading = Loading…
app-error = Error: { $message }

## Navigation
nav-dashboard = Dashboard
nav-catalog = Catalog
nav-products = Products
nav-categories = Categories
nav-categories-active = Active categories
nav-categories-archived = Archived categories
nav-categories-archived-2024 = Archive 2024
nav-sales = Sales
nav-orders = Orders
nav-customers = Customers
nav-designer = Schema designer
nav-builder = ⚙ Layout

## Table
table-empty = No records available.
table-loading = Loading table…
table-placeholder-complex = ⟨complex value⟩
table-placeholder-reference = ⟨reference⟩
table-placeholder-collection = ⟨{ $count ->
    [one] 1 item
   *[other] { $count } items
}⟩
table-actions-sort = Sort
table-actions-filter = Filter
table-actions-search = Search
table-pagination-summary = Page { $page } of { $total }
table-pagination-range = { $from }–{ $to } of { $count }
table-pagination-prev = Previous
table-pagination-next = Next

## Field (column titles)
field-id = ID
field-name = Name
field-price = Price
field-in_stock = In stock
field-created_at = Created at
field-category = Category
field-tags = Tags
field-order_number = Order number
field-total = Total
field-placed_at = Placed at
field-status = Status
field-customer = Customer
field-display_name = Display name
field-email = E-mail
field-member_since = Member since
field-order_count = Orders

## Values
value-bool-true = Yes
value-bool-false = No

## Locale switcher
locale-de = Deutsch
locale-en = English
locale-fr = Français

## Validation
validation-required = This field is required.
validation-min_length = At least { $min } characters required.
validation-max_length = At most { $max } characters allowed.
validation-number_range = Value must be between { $min } and { $max }.
validation-pattern = Value does not match the expected pattern.
validation-enum_value = Value is not one of the allowed options.

## Editor
editor-title-new = New record
editor-title-edit = Edit { $type }
editor-section-master = Master data
editor-placeholder-complex = ⟨not editable⟩
editor-actions-save = Save
editor-actions-saving = Saving…
editor-actions-cancel = Cancel
editor-actions-delete = Delete
editor-actions-back = Back
editor-state-dirty = Unsaved changes
editor-state-saved = Saved
editor-confirm-delete = Delete this record permanently?

## Error
error-decode = The response could not be decoded.
error-invalid_identifier = Invalid identifier: { $id }
error-network = Network error.
error-validation = Inputs are incomplete or invalid.
error-concurrent_modification = This record changed in the meantime. Please reload.
error-other = Unexpected error.

## Security / Login
security-group-admin = Administrators
security-group-admin-desc = Full access to every entity.
security-group-editor = Editors
security-group-editor-desc = May create and update, but not delete.
security-group-viewer = Viewers
security-group-viewer-desc = Read-only access.

login-title = Sign in
login-username = Username
login-password = Password
login-submit = Sign in
login-error-invalidCredentials = Invalid username or password.
login-error-inactive = Account is disabled.
login-error-internal = Internal error.
login-hint = Try admin/admin, editor/editor or viewer/viewer.

## Topbar
topbar-logout = Sign out
topbar-user = { $name }

## Table (extra)
table-actions-new = New
table-actions-edit = Edit
table-actions-delete = Delete
table-actions-builder = Edit layout

## Builder (Visual UI-Designer)
builder-title = Visual Builder
builder-subtitle = Entity: { $entity }
builder-forbidden = You don't have permission to use the builder.
builder-preview-title = Live preview
builder-action-add = Add node
builder-action-delete = Delete node
builder-action-undo = Undo
builder-action-redo = Redo
builder-action-add_script = Add script node
builder-script_inspector-title = Script node
builder-script_inspector-unbound = No script bound — placeholder active.
builder-action-save = Save
builder-action-reload = Load server state
builder-nodes_count = { $n } nodes
builder-status-idle = Unsaved
builder-status-loading = Loading…
builder-status-saving = Saving…
builder-status-saved = Saved (version { $version })
builder-status-conflict = Conflict — server has version { $version }
builder-status-error = Error: { $message }

## Designer
designer-title = Schema designer
designer-forbidden = You don't have permission to edit the schema.
designer-hint = Drag tables to move them. In link mode click two column ports to create a relation. Click a line to delete it.
designer-fields-schema_name = Schema name
designer-actions-add_table = Add table
designer-actions-remove_table = Remove table
designer-actions-add_column = Add column
designer-actions-remove_column = Remove column
designer-actions-save = Save schema
designer-actions-saving = Saving…
designer-actions-link_mode_off = Link mode off
designer-actions-link_mode_on = Link mode on
designer-column-add_hint = Columns
designer-column-toggle_pk = Toggle primary key
designer-port-tooltip = Click in link mode to create a relation
designer-relation-tooltip = Click to remove the relation

## Column-Editor (Q0005)
column-editor-title        = Column "{ $name }"
column-editor-visibility   = Visible
column-editor-position     = Position
column-editor-min-width    = Min width
column-editor-label        = Label
column-editor-sortable     = Sortable
column-editor-filter       = Filter
column-editor-format       = Format
column-editor-reset        = Reset
column-editor-preview      = Preview

table-actions-edit-mode    = Edit layout
table-actions-save-view    = Save
table-actions-discard-view = Discard
table-status-edit-layer    = Layer: { $layer }
table-status-pending       = { $n } unsaved changes
table-fallback-view        = View "{ $name }" not found — showing default

## Filter-Labels
filter-contains      = Contains
filter-equals        = Equals
filter-range         = Range
filter-text-contains = Contains (text)
filter-number-range  = Range (number)
filter-bool-equals   = Equals (yes/no)
filter-enum-in       = Selection
filter-date-range    = Date range

## Conflict message (Named Views)
table-view-conflict = Conflict: another edit saved in the meantime. Please reload and reapply your edits.

## Formatter-Labels
formatter-money-symbol    = EUR symbol €
formatter-money-code      = EUR code
formatter-money-decimals  = Decimals only
formatter-decimal-default = Default
formatter-decimal-2       = 2 decimals
formatter-date-iso        = ISO (YYYY-MM-DD)
formatter-date-local      = Local
formatter-datetime-iso    = ISO Date+Time
formatter-datetime-local  = Local Date+Time
formatter-int-default     = Default
formatter-bool-yesno      = Yes / No
formatter-text-plain      = Plain text

## Reference picker (U1)
picker.no_results          = No results
picker.loading             = Loading…
picker.search.placeholder  = Search…
picker.selected_label      = Selected
