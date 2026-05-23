## Application
app-title = DBlicious
app-loading = Chargement…
app-error = Erreur : { $message }

## Navigation
nav-dashboard = Tableau de bord
nav-catalog = Catalogue
nav-products = Produits
nav-categories = Catégories
nav-categories-active = Catégories actives
nav-categories-archived = Catégories archivées
nav-categories-archived-2024 = Archives 2024
nav-sales = Ventes
nav-orders = Commandes
nav-customers = Clients
nav-designer = Concepteur de schéma
nav-builder = ⚙ Mise en page

## Table
table-empty = Aucun enregistrement disponible.
table-loading = Chargement du tableau…
table-placeholder-complex = ⟨valeur complexe⟩
table-placeholder-reference = ⟨référence⟩
table-placeholder-collection = ⟨{ $count ->
    [one] 1 élément
   *[other] { $count } éléments
}⟩
table-actions-sort = Trier
table-actions-filter = Filtrer
table-actions-search = Rechercher
table-pagination-summary = Page { $page } sur { $total }
table-pagination-range = { $from }–{ $to } sur { $count }
table-pagination-prev = Précédent
table-pagination-next = Suivant

## Field (column titles)
field-id = Identifiant
field-name = Nom
field-price = Prix
field-in_stock = En stock
field-created_at = Créé le
field-category = Catégorie
field-tags = Étiquettes
field-order_number = Numéro de commande
field-total = Total
field-placed_at = Passée le
field-status = Statut
field-customer = Client
field-display_name = Nom affiché
field-email = E-mail
field-member_since = Membre depuis
field-order_count = Commandes

## Values
value-bool-true = Oui
value-bool-false = Non

## Locale switcher
locale-de = Deutsch
locale-en = English
locale-fr = Français

## Validation
validation-required = Ce champ est requis.
validation-min_length = Au moins { $min } caractères requis.
validation-max_length = Au plus { $max } caractères autorisés.
validation-number_range = La valeur doit être comprise entre { $min } et { $max }.
validation-pattern = La valeur ne correspond pas au format attendu.
validation-enum_value = Cette valeur n’est pas une option autorisée.

## Editor
editor-title-new = Nouvel enregistrement
editor-title-edit = Modifier { $type }
editor-section-master = Données principales
editor-placeholder-complex = ⟨non modifiable⟩
editor-actions-save = Enregistrer
editor-actions-saving = Enregistrement…
editor-actions-cancel = Annuler
editor-actions-delete = Supprimer
editor-actions-back = Retour
editor-state-dirty = Modifications non enregistrées
editor-state-saved = Enregistré
editor-confirm-delete = Supprimer définitivement cet enregistrement ?

## Error
error-decode = La réponse n’a pas pu être décodée.
error-invalid_identifier = Identifiant invalide : { $id }
error-network = Erreur réseau.
error-validation = Saisies incomplètes ou invalides.
error-concurrent_modification = Cet enregistrement a été modifié entre-temps. Veuillez recharger.
error-other = Erreur inattendue.

## Security / Login
security-group-admin = Administrateurs
security-group-admin-desc = Accès complet à toutes les entités.
security-group-editor = Rédacteurs
security-group-editor-desc = Peut créer et modifier, mais pas supprimer.
security-group-viewer = Lecteurs
security-group-viewer-desc = Accès en lecture seule.

login-title = Se connecter
login-username = Nom d’utilisateur
login-password = Mot de passe
login-submit = Se connecter
login-error-invalidCredentials = Nom d’utilisateur ou mot de passe invalide.
login-error-inactive = Le compte est désactivé.
login-error-internal = Erreur interne.
login-hint = Essayez admin/admin, editor/editor ou viewer/viewer.

## Topbar
topbar-logout = Se déconnecter
topbar-user = { $name }

## Table (extra)
table-actions-new = Nouveau
table-actions-edit = Modifier
table-actions-delete = Supprimer
table-actions-builder = Modifier la mise en page

## Designer
designer-title = Concepteur de schéma
designer-forbidden = Vous n’avez pas la permission de modifier le schéma.
designer-hint = Faites glisser les tables pour les déplacer. En mode lien, cliquez sur deux ports de colonne pour créer une relation. Cliquez sur une ligne pour la supprimer.
designer-fields-schema_name = Nom du schéma
designer-actions-add_table = Ajouter une table
designer-actions-remove_table = Supprimer la table
designer-actions-add_column = Ajouter une colonne
designer-actions-remove_column = Supprimer la colonne
designer-actions-save = Enregistrer le schéma
designer-actions-saving = Enregistrement…
designer-actions-link_mode_off = Mode lien désactivé
designer-actions-link_mode_on = Mode lien activé
designer-column-add_hint = Colonnes
designer-column-toggle_pk = Basculer la clé primaire
designer-port-tooltip = Cliquez en mode lien pour créer une relation
designer-relation-tooltip = Cliquez pour supprimer la relation

## Builder (Concepteur visuel)
builder-title = Concepteur visuel
builder-subtitle = Entité : { $entity }
builder-forbidden = Vous n'avez pas la permission d'utiliser le concepteur.
builder-preview-title = Aperçu en direct
builder-action-add = Ajouter un nœud
builder-action-delete = Supprimer le nœud
builder-action-undo = Annuler
builder-action-redo = Rétablir
builder-action-add_script = Ajouter un nœud de script
builder-script_inspector-title = Nœud de script
builder-script_inspector-unbound = Aucun script lié — paramètre fictif actif.
builder-action-save = Enregistrer
builder-action-reload = Charger l'état serveur
builder-nodes_count = { $n } nœuds
builder-status-idle = Non enregistré
builder-status-loading = Chargement…
builder-status-saving = Enregistrement…
builder-status-saved = Enregistré (version { $version })
builder-status-conflict = Conflit — le serveur a la version { $version }
builder-status-error = Erreur : { $message }

## Column-Editor (Q0005)
column-editor-title        = Colonne « { $name } »
column-editor-visibility   = Visible
column-editor-position     = Position
column-editor-min-width    = Largeur min
column-editor-label        = Libellé
column-editor-sortable     = Triable
column-editor-filter       = Filtre
column-editor-format       = Format
column-editor-reset        = Réinitialiser
column-editor-preview      = Aperçu

table-actions-edit-mode    = Modifier la mise en page
table-actions-save-view    = Enregistrer
table-actions-discard-view = Annuler
table-status-edit-layer    = Couche : { $layer }
table-status-pending       = { $n } modifications non enregistrées
table-fallback-view        = Vue « { $name } » introuvable — affichage par défaut

## Filter-Labels
filter-contains      = Contient
filter-equals        = Égal
filter-range         = Plage
filter-text-contains = Contient (texte)
filter-number-range  = Plage (nombre)
filter-bool-equals   = Égal (oui/non)
filter-enum-in       = Sélection
filter-date-range    = Plage de dates

## Message de conflit (vues nommées)
table-view-conflict = Conflit : une autre modification a été enregistrée entre-temps. Veuillez recharger et réappliquer vos modifications.

## Formatter-Labels
formatter-money-symbol    = Symbole € EUR
formatter-money-code      = Code EUR
formatter-money-decimals  = Décimales uniquement
formatter-decimal-default = Par défaut
formatter-decimal-2       = 2 décimales
formatter-date-iso        = ISO (AAAA-MM-JJ)
formatter-date-local      = Local
formatter-datetime-iso    = ISO Date+Heure
formatter-datetime-local  = Local Date+Heure
formatter-int-default     = Par défaut
formatter-bool-yesno      = Oui / Non
formatter-text-plain      = Texte brut
