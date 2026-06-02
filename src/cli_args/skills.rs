use clap::{Args, Subcommand};

#[derive(Args, Debug)]
#[command(after_help = "\
Skills travel inside the binary so they always match this exact
agent-desktop version. By default output is a JSON envelope on stdout
(markdown under .data.content); pass --raw to print bare markdown for
direct reading or redirecting into a file.

Examples:
  agent-desktop skills                          # List skills
  agent-desktop skills get desktop              # Primary guide (JSON envelope)
  agent-desktop skills get desktop --raw        # Primary guide as bare markdown
  agent-desktop skills get desktop --full --raw # Every reference as bare markdown
  agent-desktop skills get desktop workflows    # Single reference
  agent-desktop skills path                     # Where skills live")]
pub(crate) struct SkillsArgs {
    #[command(subcommand)]
    pub action: Option<SkillsAction>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SkillsAction {
    #[command(about = "List bundled skills with summaries (default)")]
    List,
    #[command(about = "Print a skill's markdown to stdout")]
    Get(SkillsGetArgs),
    #[command(about = "Print where bundled skills live")]
    Path,
}

#[derive(Args, Debug)]
pub(crate) struct SkillsGetArgs {
    #[arg(help = "Skill name or alias (desktop, ffi, ...)")]
    pub name: String,
    #[arg(
        help = "Reference filename (e.g. workflows or references/workflows.md). Omit for the main guide."
    )]
    pub reference: Option<String>,
    #[arg(long, help = "Append every reference file to the output")]
    pub full: bool,
    #[arg(
        long,
        help = "Print bare markdown to stdout instead of the JSON envelope"
    )]
    pub raw: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Wrap {
        #[command(subcommand)]
        action: SkillsAction,
    }

    fn parse(args: &[&str]) -> SkillsGetArgs {
        match Wrap::parse_from(args).action {
            SkillsAction::Get(g) => g,
            _ => panic!("expected get"),
        }
    }

    #[test]
    fn raw_defaults_off() {
        let g = parse(&["skills", "get", "desktop"]);
        assert!(!g.raw);
        assert!(!g.full);
    }

    #[test]
    fn raw_flag_parses_with_full_and_reference() {
        let g = parse(&["skills", "get", "desktop", "workflows", "--full", "--raw"]);
        assert!(g.raw);
        assert!(g.full);
        assert_eq!(g.reference.as_deref(), Some("workflows"));
    }
}
