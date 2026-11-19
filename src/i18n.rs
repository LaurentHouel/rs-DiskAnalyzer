use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Fr,
    Es,
    De,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Fr, Lang::Es, Lang::De];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::De => "de",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let primary = code.split(['-', '_']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    fn idx(self) -> usize {
        self as usize
    }

    fn decimal_comma(self) -> bool {
        self != Lang::En
    }
}

#[derive(Clone, Copy)]
pub enum Key {
    Folder,
    Size,
    Contents,
    Modified,
    ModifiedLabel,
    ParentTip,
    HomeTip,
    StartScreenTip,
    Open,
    OpenTip,
    RescanTip,
    Cancel,
    ScanningPath,
    ScanDetails,
    WelcomeTitle,
    WelcomeHint,
    Recent,
    Drives,
    DriveFree,
    Rings,
    Treemap,
    ShowInFileManager,
    CopyPath,
    OpenInChart,
    MoveToTrash,
    TrashTip,
    ConfirmTrash,
    TrashFailed,
    Trashed,
    MoreInfo,
    InfoPath,
    InfoType,
    KindFile,
    ShareOfParent,
    ShareOfScan,
    Close,
    MadeWith,
    MadeBy,
    GithubTip,
    DeletePermanently,
    ConfirmDelete,
    DeleteFailed,
    Deleted,
    Language,
    StatusIdle,
    StatusScanning,
    StatusDone,
    ThemeTip,
    Logs,
    LogsTip,
    LogsTitle,
    LogsEmpty,
    AppStarted,
    LogScanStarted,
    LogScanCancelled,
    LogScanCompleted,
    LogScanNoProgress,
    ScanActivityCheckingRoot,
    ScanActivityReadingDirectory,
    ScanActivityCheckingEntries,
    UpdateAvailable,
    UpdateAvailableTip,
    UpdateCheckFailed,
    Bytes,
    Today,
    Yesterday,
    Day1,
    DayN,
    Month1,
    MonthN,
    Year1,
    YearN,
    Item1,
    ItemN,
}

pub fn tr(lang: Lang, key: Key) -> &'static str {
    use Key::*;
    let t: [&'static str; 4] = match key {
        Folder => ["Folder", "Dossier", "Carpeta", "Ordner"],
        Size => ["Size", "Taille", "Tamaño", "Größe"],
        Contents => ["Contents", "Contenu", "Contenido", "Inhalt"],
        Modified => ["Modified", "Modifié", "Modificado", "Geändert"],
        ModifiedLabel => ["Modified:", "Modifié :", "Modificado:", "Geändert:"],
        ParentTip => [
            "Parent folder (Backspace)",
            "Dossier parent (Retour arrière)",
            "Carpeta superior (Retroceso)",
            "Übergeordneter Ordner (Rücktaste)",
        ],
        StartScreenTip => [
            "Back to the selection screen",
            "Retour à l’écran de sélection",
            "Volver a la pantalla de selección",
            "Zurück zur Auswahl",
        ],
        HomeTip => [
            "Scan home folder",
            "Analyser le dossier personnel",
            "Analizar la carpeta personal",
            "Persönlichen Ordner analysieren",
        ],
        Open => ["Open…", "Ouvrir…", "Abrir…", "Öffnen…"],
        OpenTip => [
            "Choose a folder to scan",
            "Choisir un dossier à analyser",
            "Elegir una carpeta para analizar",
            "Ordner zum Analysieren wählen",
        ],
        RescanTip => ["Rescan", "Analyser à nouveau", "Volver a analizar", "Erneut analysieren"],
        Cancel => ["Cancel", "Annuler", "Cancelar", "Abbrechen"],
        ScanningPath => ["Scanning {0}…", "Analyse de {0}…", "Analizando {0}…", "Analysiere {0}…"],
        ScanDetails => [
            "{0} folders · {1} files · {2} · {3}s",
            "{0} dossiers · {1} fichiers · {2} · {3} s",
            "{0} carpetas · {1} archivos · {2} · {3} s",
            "{0} Ordner · {1} Dateien · {2} · {3} s",
        ],
        WelcomeTitle => ["Disk Analyzer", "Analyseur de disque", "Analizador de disco", "Festplattenanalyse"],
        WelcomeHint => [
            "Choose what to scan. Nothing is scanned until you ask.",
            "Choisissez ce qu’il faut analyser. Rien n’est analysé sans votre demande.",
            "Elija qué analizar. No se analiza nada hasta que lo pida.",
            "Wählen Sie, was analysiert werden soll. Es wird nichts ohne Ihre Aufforderung gescannt.",
        ],
        Drives => ["Drives", "Disques", "Discos", "Laufwerke"],
        DriveFree => [
            "{0} free of {1}",
            "{0} libres sur {1}",
            "{0} libres de {1}",
            "{0} frei von {1}",
        ],
        Recent => ["Recent folders", "Dossiers récents", "Carpetas recientes", "Zuletzt verwendet"],
        Rings => ["Rings Chart", "Anneaux", "Anillos", "Ringdiagramm"],
        Treemap => ["Treemap Chart", "Treemap", "Treemap", "Treemap"],
        ShowInFileManager => [
            "Show in file manager",
            "Afficher dans le gestionnaire de fichiers",
            "Mostrar en el gestor de archivos",
            "Im Dateimanager anzeigen",
        ],
        CopyPath => ["Copy path", "Copier le chemin", "Copiar la ruta", "Pfad kopieren"],
        OpenInChart => [
            "Open in chart",
            "Ouvrir dans le graphique",
            "Abrir en el gráfico",
            "Im Diagramm öffnen",
        ],
        MoveToTrash => [
            "Move to trash",
            "Mettre à la corbeille",
            "Mover a la papelera",
            "In den Papierkorb verschieben",
        ],
        TrashTip => [
            "Move the selection to the trash (Delete)",
            "Mettre la sélection à la corbeille (Suppr)",
            "Mover la selección a la papelera (Supr)",
            "Auswahl in den Papierkorb verschieben (Entf)",
        ],
        ConfirmTrash => [
            "Move “{0}” ({1}) to the trash?",
            "Mettre « {0} » ({1}) à la corbeille ?",
            "¿Mover «{0}» ({1}) a la papelera?",
            "„{0}“ ({1}) in den Papierkorb verschieben?",
        ],
        TrashFailed => [
            "Could not move to trash: {0}",
            "Impossible de mettre à la corbeille : {0}",
            "No se pudo mover a la papelera: {0}",
            "Verschieben in den Papierkorb fehlgeschlagen: {0}",
        ],
        Trashed => [
            "“{0}” moved to the trash",
            "« {0} » mis à la corbeille",
            "«{0}» movido a la papelera",
            "„{0}“ in den Papierkorb verschoben",
        ],
        MoreInfo => [
            "More information…",
            "Plus d’informations…",
            "Más información…",
            "Weitere Informationen…",
        ],
        InfoPath => ["Path", "Chemin", "Ruta", "Pfad"],
        InfoType => ["Type", "Type", "Tipo", "Typ"],
        KindFile => ["File", "Fichier", "Archivo", "Datei"],
        ShareOfParent => [
            "Share of parent folder",
            "Part du dossier parent",
            "Parte de la carpeta superior",
            "Anteil am übergeordneten Ordner",
        ],
        ShareOfScan => [
            "Share of the whole scan",
            "Part de l’analyse complète",
            "Parte del análisis completo",
            "Anteil an der gesamten Analyse",
        ],
        MadeWith => [
            "Made with",
            "Fait avec",
            "Hecho con",
            "Mit",
        ],
        MadeBy => [
            "by lowlowell",
            "par lowlowell",
            "por lowlowell",
            "gemacht von lowlowell",
        ],
        GithubTip => [
            "Open the project on GitHub",
            "Ouvrir le projet sur GitHub",
            "Abrir el proyecto en GitHub",
            "Projekt auf GitHub öffnen",
        ],
        Close => ["Close", "Fermer", "Cerrar", "Schließen"],
        DeletePermanently => [
            "Delete permanently",
            "Supprimer définitivement",
            "Eliminar definitivamente",
            "Endgültig löschen",
        ],
        ConfirmDelete => [
            "Permanently delete “{0}” ({1})? This cannot be undone.",
            "Supprimer définitivement « {0} » ({1}) ? Cette action est irréversible.",
            "¿Eliminar definitivamente «{0}» ({1})? Esta acción no se puede deshacer.",
            "„{0}“ ({1}) endgültig löschen? Das kann nicht rückgängig gemacht werden.",
        ],
        DeleteFailed => [
            "Could not delete: {0}",
            "Impossible de supprimer : {0}",
            "No se pudo eliminar: {0}",
            "Löschen fehlgeschlagen: {0}",
        ],
        Deleted => [
            "“{0}” deleted permanently",
            "« {0} » supprimé définitivement",
            "«{0}» eliminado definitivamente",
            "„{0}“ endgültig gelöscht",
        ],
        Language => ["Language", "Langue", "Idioma", "Sprache"],
        StatusIdle => [
            "Idle — Ready to scan",
            "Inactif — Prêt à analyser",
            "Inactivo — Listo para analizar",
            "Leerlauf — Bereit zum Analysieren",
        ],
        StatusScanning => [
            "Scanning — {0} files · {1}",
            "Analyse — {0} fichiers · {1}",
            "Analizando — {0} archivos · {1}",
            "Analyse — {0} Dateien · {1}",
        ],
        StatusDone => [
            "Scan complete — {0} · {1}",
            "Analyse terminée — {0} · {1}",
            "Análisis completo — {0} · {1}",
            "Analyse abgeschlossen — {0} · {1}",
        ],
        ThemeTip => [
            "Switch between light and dark theme",
            "Basculer entre thème clair et sombre",
            "Cambiar entre tema claro y oscuro",
            "Zwischen hellem und dunklem Design wechseln",
        ],
        Logs => ["Logs", "Logs", "Registros", "Protokoll"],
        LogsTip => [
            "Show activity logs",
            "Afficher le journal d’activité",
            "Mostrar el registro de actividad",
            "Aktivitätsprotokoll anzeigen",
        ],
        LogsTitle => ["Activity logs", "Journal d’activité", "Registro de actividad", "Aktivitätsprotokoll"],
        LogsEmpty => ["No activity yet", "Aucune activité pour le moment", "Aún no hay actividad", "Noch keine Aktivität"],
        AppStarted => [
            "Application started",
            "Application démarrée",
            "Aplicación iniciada",
            "Anwendung gestartet",
        ],
        LogScanStarted => [
            "Scan started: {0}",
            "Analyse démarrée : {0}",
            "Análisis iniciado: {0}",
            "Analyse gestartet: {0}",
        ],
        LogScanCancelled => [
            "Scan cancelled: {0}",
            "Analyse annulée : {0}",
            "Análisis cancelado: {0}",
            "Analyse abgebrochen: {0}",
        ],
        LogScanCompleted => [
            "Scan completed: {0} · {1} folders · {2} files · {3} · {4} errors · {5}",
            "Analyse terminée : {0} · {1} dossiers · {2} fichiers · {3} · {4} erreurs · {5}",
            "Análisis completado: {0} · {1} carpetas · {2} archivos · {3} · {4} errores · {5}",
            "Analyse abgeschlossen: {0} · {1} Ordner · {2} Dateien · {3} · {4} Fehler · {5}",
        ],
        LogScanNoProgress => [
            "No progress for {0} · {1} folders · {2} files · {3}",
            "Aucune progression depuis {0} · {1} dossiers · {2} fichiers · {3}",
            "Sin progreso durante {0} · {1} carpetas · {2} archivos · {3}",
            "Kein Fortschritt seit {0} · {1} Ordner · {2} Dateien · {3}",
        ],
        ScanActivityCheckingRoot => [
            "Checking selected path: {0}",
            "Vérification du chemin choisi : {0}",
            "Comprobando la ruta seleccionada: {0}",
            "Ausgewählten Pfad prüfen: {0}",
        ],
        ScanActivityReadingDirectory => [
            "Reading folder: {0}",
            "Lecture du dossier : {0}",
            "Leyendo carpeta: {0}",
            "Ordner wird gelesen: {0}",
        ],
        ScanActivityCheckingEntries => [
            "Checking entries in: {0}",
            "Vérification des éléments dans : {0}",
            "Comprobando elementos en: {0}",
            "Einträge werden geprüft in: {0}",
        ],
        UpdateAvailable => [
            "A newer release ({0}) is available",
            "Une nouvelle version ({0}) est disponible",
            "Hay una nueva versión ({0}) disponible",
            "Eine neuere Version ({0}) ist verfügbar",
        ],
        UpdateAvailableTip => [
            "Open release {0} on GitHub",
            "Ouvrir la version {0} sur GitHub",
            "Abrir la versión {0} en GitHub",
            "Version {0} auf GitHub öffnen",
        ],
        UpdateCheckFailed => [
            "Could not check for updates: {0}",
            "Vérification des mises à jour impossible : {0}",
            "No se pudieron comprobar las actualizaciones: {0}",
            "Update-Prüfung fehlgeschlagen: {0}",
        ],
        Bytes => ["bytes", "octets", "bytes", "Bytes"],
        Today => ["Today", "Aujourd’hui", "Hoy", "Heute"],
        Yesterday => ["Yesterday", "Hier", "Ayer", "Gestern"],
        Day1 => ["1 day", "1 jour", "1 día", "1 Tag"],
        DayN => ["{0} days", "{0} jours", "{0} días", "{0} Tage"],
        Month1 => ["1 month", "1 mois", "1 mes", "1 Monat"],
        MonthN => ["{0} months", "{0} mois", "{0} meses", "{0} Monate"],
        Year1 => ["1 year", "1 an", "1 año", "1 Jahr"],
        YearN => ["{0} years", "{0} ans", "{0} años", "{0} Jahre"],
        Item1 => ["1 item", "1 élément", "1 elemento", "1 Element"],
        ItemN => ["{0} items", "{0} éléments", "{0} elementos", "{0} Elemente"],
    };
    t[lang.idx()]
}

/// Translates `key` and substitutes `{0}`, `{1}`… with `args`.
pub fn trf(lang: Lang, key: Key, args: &[&str]) -> String {
    let mut s = tr(lang, key).to_owned();
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), a);
    }
    s
}

pub fn format_size(lang: Lang, bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} {}", tr(lang, Key::Bytes));
    }
    let units: [&str; 5] = if lang == Lang::Fr {
        ["ko", "Mo", "Go", "To", "Po"]
    } else {
        ["kB", "MB", "GB", "TB", "PB"]
    };
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() {
        value /= 1000.0;
        unit += 1;
    }
    let mut num = format!("{value:.1}");
    if lang.decimal_comma() {
        num = num.replace('.', ",");
    }
    format!("{num} {}", units[unit - 1])
}

/// Compact scan duration: "0.8 s", "42 s", "3 min 07 s".
pub fn format_duration(lang: Lang, d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 10.0 {
        let num = format!("{secs:.1}");
        let num = if lang.decimal_comma() { num.replace('.', ",") } else { num };
        format!("{num} s")
    } else if secs < 60.0 {
        format!("{:.0} s", secs)
    } else {
        let total = secs.round() as u64;
        format!("{} min {:02} s", total / 60, total % 60)
    }
}

pub fn format_items(lang: Lang, n: u64) -> String {
    if n == 1 {
        tr(lang, Key::Item1).to_owned()
    } else {
        trf(lang, Key::ItemN, &[&n.to_string()])
    }
}

pub fn format_age(lang: Lang, time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };
    let days = SystemTime::now()
        .duration_since(time)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    let plural = |n: u64, one: Key, many: Key| {
        if n == 1 {
            tr(lang, one).to_owned()
        } else {
            trf(lang, many, &[&n.to_string()])
        }
    };
    match days {
        0 => tr(lang, Key::Today).to_owned(),
        1 => tr(lang, Key::Yesterday).to_owned(),
        2..=29 => plural(days, Key::Day1, Key::DayN),
        30..=364 => plural(days / 30, Key::Month1, Key::MonthN),
        _ => plural(days / 365, Key::Year1, Key::YearN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_localized() {
        assert_eq!(format_size(Lang::En, 999), "999 bytes");
        assert_eq!(format_size(Lang::En, 53_200_000_000), "53.2 GB");
        assert_eq!(format_size(Lang::Fr, 1_500_000), "1,5 Mo");
    }

    #[test]
    fn durations_are_compact_and_localized() {
        assert_eq!(format_duration(Lang::En, Duration::from_millis(840)), "0.8 s");
        assert_eq!(format_duration(Lang::Fr, Duration::from_millis(2_450)), "2,5 s");
        assert_eq!(format_duration(Lang::En, Duration::from_secs(42)), "42 s");
        assert_eq!(format_duration(Lang::En, Duration::from_secs(187)), "3 min 07 s");
    }

    #[test]
    fn language_codes_and_placeholders() {
        assert_eq!(Lang::from_code("fr_FR.UTF-8"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("pt-BR"), None);
        assert_eq!(
            trf(Lang::De, Key::ScanDetails, &["1", "2", "3", "4"]),
            "1 Ordner · 2 Dateien · 3 · 4 s"
        );
    }
}
