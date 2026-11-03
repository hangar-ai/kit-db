//! The tab's words in English and French, `logic.js`'s `STRINGS` ported: a count `n` picks `key_one`
//! or `key_other`, a language lacking a key falls back on English, and the SDK sets the
//! typography (no-break spaces) as Hangar UI does.

use hangar_kit_sdk::i18n::Strings;

/// English, and what every other language falls back on.
pub const EN: &[(&str, &str)] = &[
    ("tab", "Database"),
    ("readOnly", "Read-only"),
    ("refresh", "Refresh"),
    ("databaseLabel", "Database"),
    ("loading", "Reading the database…"),
    ("noDbTitle", "No database found"),
    (
        "noDbText",
        "This tab looks for .db, .sqlite, .sqlite3 and .db3 files in the workspace folder and opens them read-only. Create one, or ask an agent to open one here.",
    ),
    (
        "unusable",
        "Not openable: its path has characters this kit does not accept.",
    ),
    ("lockedTitle", "The database is locked"),
    (
        "lockedText",
        "Another program is writing to it. Wait until it is done, then try again. Nothing was changed.",
    ),
    ("noSqliteTitle", "No SQLite reader on this computer"),
    (
        "noSqliteText",
        "The kit reads databases with Node.js 22.12 or later, or else with the sqlite3 program (3.33 or later). Install one of them, then try again.",
    ),
    ("deniedTitle", "This tab may not read the database"),
    (
        "deniedText",
        "Accept command:list, command:tables, command:schema and command:rows in Settings › Kits, then try again. The tab only runs the read-only script it declares.",
    ),
    ("notDatabaseTitle", "This file is not a SQLite database"),
    (
        "notDatabaseText",
        "Its header is not SQLite's, or it is encrypted. Pick another file.",
    ),
    ("goneTitle", "This file is not in the workspace any more"),
    ("goneText", "Refresh the list, then pick another."),
    ("errorTitle", "The database could not be read"),
    (
        "error.timeout",
        "It took longer than the time allowed. Try again.",
    ),
    (
        "error.not_found",
        "Node.js was not found on this computer. Install it, then try again.",
    ),
    (
        "error.busy",
        "Hangar is already running the most commands it allows. Try again in a moment.",
    ),
    (
        "error.unreadable",
        "The script printed something that is not an answer. Reload the tab, then try again.",
    ),
    (
        "error.bad-table",
        "This database has no such table any more.",
    ),
    (
        "error.bad-args",
        "The kit refused that request. Reload the tab, then try again.",
    ),
    (
        "error.outside",
        "That path leads outside the workspace, which the kit never opens.",
    ),
    ("error.failed", "The script could not read it: {detail}"),
    ("retry", "Try again"),
    ("tables", "Tables"),
    ("view", "view"),
    ("rowsCount_one", "{n} row"),
    ("rowsCount_other", "{n} rows"),
    ("columnsCount_one", "{n} column"),
    ("columnsCount_other", "{n} columns"),
    ("unsupportedName", "Name not supported by this kit"),
    ("noTables", "This database has no table."),
    ("structure", "Structure"),
    ("indexes", "Indexes"),
    ("key", "key"),
    ("required", "required"),
    ("defaultValue", "default {value}"),
    ("unique", "unique"),
    ("columnsCut", "Only the first 200 columns are shown."),
    ("columnsShown", "Only the first {n} columns are shown."),
    ("rowsTitle", "Rows"),
    ("range", "{from}–{to} of {total}"),
    ("previous", "Previous"),
    ("next", "Next"),
    ("emptyTable", "This table has no row."),
    ("null", "NULL"),
    ("blob_one", "{n} byte (binary)"),
    ("blob_other", "{n} bytes (binary)"),
    ("showFull", "Show all {n} characters"),
    ("showLess", "Show less"),
    ("loadingFull", "Reading…"),
    ("fullFailed", "The whole value could not be read."),
    ("fullCut", "Shown up to {n} characters."),
    ("capped", "The page was shortened to keep the answer small."),
    (
        "secretNote",
        "Values may be secrets. They stay on this screen: nothing goes to an agent unless you click “Hand over to an agent”, and then only the table and your question, never its rows.",
    ),
    ("handOver", "Hand over to an agent"),
    ("handOverTitle", "Hand over this table"),
    ("questionLabel", "Your question"),
    (
        "questionHint",
        "Sent with the table’s name and columns, never its rows.",
    ),
    (
        "questionPlaceholder",
        "What do you want the agent to find out?",
    ),
    ("agentLabel", "Agent"),
    ("send", "Send"),
    ("sending", "Sending…"),
    ("cancel", "Cancel"),
    ("sentTo", "Handed to {agent}"),
    ("noAgent", "No Claude Code agent can receive text now."),
    (
        "panesDenied",
        "This tab may not list the agents. Accept panes.read in Settings › Kits to hand a table over.",
    ),
    (
        "problem.denied",
        "This tab may not send to agents. Accept panes.send in Settings › Kits.",
    ),
    (
        "problem.not_found",
        "{agent} is not in this workspace any more.",
    ),
    (
        "problem.rate_limited",
        "Too many sends in a minute. Try again in a moment.",
    ),
    (
        "problem.unavailable",
        "{agent} cannot receive text now. Pick another agent, or try again.",
    ),
    ("problem.failed", "It could not be sent: {detail}"),
];

pub const FR: &[(&str, &str)] = &[
    ("tab", "Base de données"),
    ("readOnly", "Lecture seule"),
    ("refresh", "Actualiser"),
    ("databaseLabel", "Base de données"),
    ("loading", "Lecture de la base…"),
    ("noDbTitle", "Aucune base de données trouvée"),
    (
        "noDbText",
        "Cet onglet cherche les fichiers .db, .sqlite, .sqlite3 et .db3 dans le dossier de l’espace de travail et les ouvre en lecture seule. Créez-en une, ou demandez à un agent d’en ouvrir une ici.",
    ),
    (
        "unusable",
        "Ouverture impossible : son chemin contient des caractères que ce kit n’accepte pas.",
    ),
    ("lockedTitle", "La base de données est verrouillée"),
    (
        "lockedText",
        "Un autre programme y écrit. Attendez qu’il ait fini, puis réessayez. Rien n’a été modifié.",
    ),
    ("noSqliteTitle", "Aucun lecteur SQLite sur cet ordinateur"),
    (
        "noSqliteText",
        "Le kit lit les bases avec Node.js 22.12 ou plus récent, ou à défaut avec le programme sqlite3 (3.33 ou plus récent). Installez l’un des deux, puis réessayez.",
    ),
    ("deniedTitle", "Cet onglet n’a pas le droit de lire la base"),
    (
        "deniedText",
        "Acceptez command:list, command:tables, command:schema et command:rows dans Réglages › Kits, puis réessayez. L’onglet ne lance que le script en lecture seule qu’il déclare.",
    ),
    ("notDatabaseTitle", "Ce fichier n’est pas une base SQLite"),
    (
        "notDatabaseText",
        "Son en-tête n’est pas celui de SQLite, ou il est chiffré. Choisissez un autre fichier.",
    ),
    (
        "goneTitle",
        "Ce fichier n’est plus dans l’espace de travail",
    ),
    (
        "goneText",
        "Actualisez la liste, puis choisissez-en un autre.",
    ),
    ("errorTitle", "La base n’a pas pu être lue"),
    (
        "error.timeout",
        "La lecture a pris plus de temps que prévu. Réessayez.",
    ),
    (
        "error.not_found",
        "Node.js est introuvable sur cet ordinateur. Installez-le, puis réessayez.",
    ),
    (
        "error.busy",
        "Hangar lance déjà le maximum de commandes autorisé. Réessayez dans un instant.",
    ),
    (
        "error.unreadable",
        "Le script a affiché autre chose qu’une réponse. Rechargez l’onglet, puis réessayez.",
    ),
    ("error.bad-table", "Cette base n’a plus de table de ce nom."),
    (
        "error.bad-args",
        "Le kit a refusé cette demande. Rechargez l’onglet, puis réessayez.",
    ),
    (
        "error.outside",
        "Ce chemin sort de l’espace de travail, que le kit n’ouvre jamais.",
    ),
    ("error.failed", "Le script n’a pas pu la lire : {detail}"),
    ("retry", "Réessayer"),
    ("tables", "Tables"),
    ("view", "vue"),
    ("rowsCount_one", "{n} ligne"),
    ("rowsCount_other", "{n} lignes"),
    ("columnsCount_one", "{n} colonne"),
    ("columnsCount_other", "{n} colonnes"),
    ("unsupportedName", "Nom non pris en charge par ce kit"),
    ("noTables", "Cette base ne contient aucune table."),
    ("structure", "Structure"),
    ("indexes", "Index"),
    ("key", "clé"),
    ("required", "obligatoire"),
    ("defaultValue", "défaut {value}"),
    ("unique", "unique"),
    (
        "columnsCut",
        "Seules les 200 premières colonnes sont affichées.",
    ),
    (
        "columnsShown",
        "Seules les {n} premières colonnes sont affichées.",
    ),
    ("rowsTitle", "Lignes"),
    ("range", "{from}–{to} sur {total}"),
    ("previous", "Précédent"),
    ("next", "Suivant"),
    ("emptyTable", "Cette table ne contient aucune ligne."),
    ("null", "NULL"),
    ("blob_one", "{n} octet (binaire)"),
    ("blob_other", "{n} octets (binaire)"),
    ("showFull", "Afficher les {n} caractères"),
    ("showLess", "Réduire"),
    ("loadingFull", "Lecture…"),
    ("fullFailed", "La valeur entière n’a pas pu être lue."),
    ("fullCut", "Affichée jusqu’à {n} caractères."),
    (
        "capped",
        "La page a été raccourcie pour garder une réponse légère.",
    ),
    (
        "secretNote",
        "Les valeurs peuvent être des secrets. Elles restent sur cet écran : rien n’est envoyé à un agent tant que vous ne cliquez pas sur « Confier à un agent », et alors seulement la table et votre question, jamais ses lignes.",
    ),
    ("handOver", "Confier à un agent"),
    ("handOverTitle", "Confier cette table"),
    ("questionLabel", "Votre question"),
    (
        "questionHint",
        "Envoyée avec le nom de la table et ses colonnes, jamais ses lignes.",
    ),
    (
        "questionPlaceholder",
        "Que voulez-vous que l’agent cherche ?",
    ),
    ("agentLabel", "Agent"),
    ("send", "Envoyer"),
    ("sending", "Envoi…"),
    ("cancel", "Annuler"),
    ("sentTo", "Confié à {agent}"),
    (
        "noAgent",
        "Aucun agent Claude Code ne peut recevoir de texte pour l’instant.",
    ),
    (
        "panesDenied",
        "Cet onglet n’a pas le droit de lister les agents. Acceptez panes.read dans Réglages › Kits pour confier une table.",
    ),
    (
        "problem.denied",
        "Cet onglet n’a pas le droit d’écrire aux agents. Acceptez panes.send dans Réglages › Kits.",
    ),
    (
        "problem.not_found",
        "{agent} n’est plus dans cet espace de travail.",
    ),
    (
        "problem.rate_limited",
        "Trop d’envois en une minute. Réessayez dans un instant.",
    ),
    (
        "problem.unavailable",
        "{agent} ne peut pas recevoir de texte pour l’instant. Choisissez un autre agent, ou réessayez.",
    ),
    ("problem.failed", "L’envoi a échoué : {detail}"),
];

/// The tab's strings, speaking `locale`.
pub fn strings(locale: &str) -> Strings {
    Strings::new(locale).add("en", EN).add("fr", FR)
}
