/// The eight apps, in PRD order, plus the one window that holds them all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppId {
    Doc,
    Table,
    Show,
    Pdf,
    Mail,
    Tree,
    Mark,
    Agent,
    /// MPelyoffice: the one window with every document kind in one tab strip.
    Office,
}

impl AppId {
    pub const ALL: [AppId; 9] = [
        AppId::Doc,
        AppId::Table,
        AppId::Show,
        AppId::Pdf,
        AppId::Mail,
        AppId::Tree,
        AppId::Mark,
        AppId::Agent,
        AppId::Office,
    ];

    /// The macOS bundle identifier `packaging/macos/bundle.sh` writes.
    pub fn bundle_id(self) -> String {
        let slug = self.slug();
        let suffix = slug.strip_prefix("ely-").unwrap_or(slug);
        format!("ely.zacharyzhang.office.{suffix}")
    }

    pub fn name(self) -> &'static str {
        match self {
            AppId::Doc => "ElyDoc",
            AppId::Table => "ElyTable",
            AppId::Show => "ElyShow",
            AppId::Pdf => "ElyPDF",
            AppId::Mail => "ElyMail",
            AppId::Tree => "ElyTree",
            AppId::Mark => "ElyMark",
            AppId::Agent => "ElyAgent",
            AppId::Office => "MPelyoffice",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            AppId::Doc => "ely-doc",
            AppId::Table => "ely-table",
            AppId::Show => "ely-show",
            AppId::Pdf => "ely-pdf",
            AppId::Mail => "ely-mail",
            AppId::Tree => "ely-tree",
            AppId::Mark => "ely-mark",
            AppId::Agent => "ely-agent",
            AppId::Office => "mpelyoffice",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppId::{self, *};

    const IN_PRD_ORDER: [AppId; 9] = AppId::ALL;

    #[test]
    fn bundle_ids_match_the_packaging() {
        assert_eq!(Doc.bundle_id(), "ely.zacharyzhang.office.doc");
        let script = include_str!("../../../packaging/macos/bundle.sh");
        assert!(script.contains("prefix=ely.zacharyzhang.office\n"));
        assert!(script.contains("<string>$prefix.${slug#ely-}</string>"));
    }

    #[test]
    fn names_follow_the_prd() {
        let names: Vec<_> = IN_PRD_ORDER.iter().map(|app| app.name()).collect();
        assert_eq!(
            names,
            [
                "ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyMail", "ElyTree", "ElyMark",
                "ElyAgent", "MPelyoffice"
            ]
        );
    }

    #[test]
    fn slugs_are_the_binary_names() {
        let slugs: Vec<_> = IN_PRD_ORDER.iter().map(|app| app.slug()).collect();
        assert_eq!(
            slugs,
            [
                "ely-doc",
                "ely-table",
                "ely-show",
                "ely-pdf",
                "ely-mail",
                "ely-tree",
                "ely-mark",
                "ely-agent",
                "mpelyoffice"
            ]
        );
    }

    #[test]
    fn the_one_window_has_its_own_name_and_slug() {
        assert_eq!(Office.name(), "MPelyoffice");
        assert_eq!(Office.slug(), "mpelyoffice");
        assert_eq!(Office.bundle_id(), "ely.zacharyzhang.office.mpelyoffice");
    }
}
