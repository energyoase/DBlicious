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
