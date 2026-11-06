pub mod fit;
pub mod materials;
pub mod oring;
pub mod tables;
pub mod thread;

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub title: String,
    pub rows: Vec<(String, String)>,
    pub why: String,
    pub formulas: Vec<String>,
    pub sources: Vec<String>,
}

impl Report {
    pub fn render(&self, with_formulas: bool) -> String {
        let width = self.rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        let mut out = vec![self.title.clone()];
        out.extend(self.rows.iter().map(|(k, v)| format!("  {k:<width$}  {v}")));
        out.push(format!("why: {}", self.why));
        if with_formulas {
            out.push("formulas:".into());
            out.extend(self.formulas.iter().map(|f| format!("  {f}")));
            out.push("source:".into());
            out.extend(self.sources.iter().map(|s| format!("  {s}")));
        }
        out.join("\n")
    }
}
