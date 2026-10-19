/// What to open: a range or folder, the staged changes, a pull request or a patch on stdin.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Subject {
    pub revision: Option<String>,
    pub staged: bool,
    pub stdin: bool,
    pub pull_request: Option<u32>,
    pub paths: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Open {
        subject: Subject,
        agent: Option<String>,
    },
    Review {
        subject: Subject,
        agent: Option<String>,
        print: bool,
        last: bool,
    },
    Explain {
        revision: String,
        agent: Option<String>,
        print: bool,
    },
    Doctor,
    GitInstall {
        default: bool,
        uninstall: bool,
    },
    Difftool {
        left: String,
        right: String,
        merged: Option<String>,
    },
}

#[derive(Default)]
struct Flags {
    subject: Subject,
    agent: Option<String>,
    print: bool,
    last: bool,
    help: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Allowed {
    Open,
    Review,
    Explain,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let command = match args.split_first() {
        Some((&("-h" | "--help" | "help"), _)) => Command::Help,
        Some((&("-V" | "--version"), [])) => Command::Version,
        Some((&"doctor", rest)) => {
            no_more("doctor", rest)?;
            Command::Doctor
        }
        Some((&"git-install", rest)) => git_install(rest)?,
        Some((&"--difftool", rest)) => match rest {
            [left, right] => Command::Difftool {
                left: left.to_string(),
                right: right.to_string(),
                merged: None,
            },
            [left, right, merged] => Command::Difftool {
                left: left.to_string(),
                right: right.to_string(),
                merged: (!merged.is_empty()).then(|| merged.to_string()),
            },
            _ => return Err("--difftool takes the two sides git passes it".into()),
        },
        Some((&"review", rest)) => {
            let flags = flags(rest, Allowed::Review)?;
            if flags.help {
                return Ok(Command::Help);
            }
            if flags.last && (flags.print || flags.subject != Subject::default()) {
                return Err("--last opens the saved review as it is; it takes nothing else".into());
            }
            Command::Review {
                subject: flags.subject,
                agent: flags.agent,
                print: flags.print,
                last: flags.last,
            }
        }
        Some((&"explain", rest)) => {
            let flags = flags(rest, Allowed::Explain)?;
            if flags.help {
                return Ok(Command::Help);
            }
            let revision = flags
                .subject
                .revision
                .ok_or("explain needs a commit, e.g. `seam explain HEAD`")?;
            Command::Explain {
                revision,
                agent: flags.agent,
                print: flags.print,
            }
        }
        _ => {
            let flags = flags(&args, Allowed::Open)?;
            if flags.help {
                return Ok(Command::Help);
            }
            Command::Open {
                subject: flags.subject,
                agent: flags.agent,
            }
        }
    };
    Ok(command)
}

fn no_more(command: &str, rest: &[&str]) -> Result<(), String> {
    match rest.first() {
        None => Ok(()),
        Some(extra) => Err(format!("{command} takes no arguments, got `{extra}`")),
    }
}

fn git_install(rest: &[&str]) -> Result<Command, String> {
    let (mut default, mut uninstall) = (false, false);
    for argument in rest {
        match *argument {
            "--default" => default = true,
            "--uninstall" => uninstall = true,
            other => return Err(format!("git-install doesn't know `{other}`")),
        }
    }
    if default && uninstall {
        return Err("--default and --uninstall don't go together".into());
    }
    Ok(Command::GitInstall { default, uninstall })
}

fn flags(args: &[&str], allowed: Allowed) -> Result<Flags, String> {
    let mut flags = Flags::default();
    let mut rest = args.iter();
    while let Some(&argument) = rest.next() {
        match argument {
            "--" => {
                flags.subject.paths = rest.map(|path| path.to_string()).collect();
                break;
            }
            "-h" | "--help" => flags.help = true,
            "-" if allowed != Allowed::Explain => flags.subject.stdin = true,
            "--staged" | "--cached" if allowed != Allowed::Explain => flags.subject.staged = true,
            "--print" if allowed != Allowed::Open => flags.print = true,
            "--last" if allowed == Allowed::Review => flags.last = true,
            "--agent" => {
                let agent = rest
                    .next()
                    .ok_or("--agent needs a name, e.g. --agent claude")?;
                flags.agent = Some(agent.to_string());
            }
            _ if argument.starts_with("--agent=") => {
                flags.agent = Some(argument["--agent=".len()..].to_string());
            }
            "pr" if allowed != Allowed::Explain
                && flags.subject.revision.is_none()
                && flags.subject.pull_request.is_none() =>
            {
                let number = rest.next().ok_or("pr needs a number, e.g. `seam pr 128`")?;
                let number = number
                    .trim_start_matches(['#', '!'])
                    .parse()
                    .map_err(|_| format!("`{number}` isn't a pull request number"))?;
                flags.subject.pull_request = Some(number);
            }
            _ if argument.starts_with('-') => return Err(format!("unknown option `{argument}`")),
            _ if flags.subject.revision.is_none() => {
                flags.subject.revision = Some(argument.to_string());
            }
            _ => {
                return Err(format!(
                    "one range at a time, got `{argument}` too (paths go after `--`)"
                ))
            }
        }
    }
    let subject = &flags.subject;
    let sources = [
        subject.revision.is_some(),
        subject.staged,
        subject.stdin,
        subject.pull_request.is_some(),
    ];
    if sources.iter().filter(|&&given| given).count() > 1 {
        return Err("pick one of a range, --staged, `pr <n>` or `-`".into());
    }
    if !subject.paths.is_empty() && (subject.stdin || subject.pull_request.is_some()) {
        return Err("paths after `--` only narrow a range or the working tree".into());
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_line(line: &str) -> Result<Command, String> {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        parse(&args)
    }

    fn open(subject: Subject) -> Command {
        Command::Open {
            subject,
            agent: None,
        }
    }

    #[test]
    fn opens_every_kind_of_change() {
        assert_eq!(parse_line("").unwrap(), open(Subject::default()));
        assert_eq!(
            parse_line("--staged").unwrap(),
            open(Subject {
                staged: true,
                ..Default::default()
            })
        );
        assert_eq!(
            parse_line("main..HEAD").unwrap(),
            open(Subject {
                revision: Some("main..HEAD".into()),
                ..Default::default()
            })
        );
        assert_eq!(
            parse_line("HEAD~3 -- src docs").unwrap(),
            open(Subject {
                revision: Some("HEAD~3".into()),
                paths: vec!["src".into(), "docs".into()],
                ..Default::default()
            })
        );
        assert_eq!(
            parse_line("pr 128").unwrap(),
            open(Subject {
                pull_request: Some(128),
                ..Default::default()
            })
        );
        assert_eq!(
            parse_line("pr !42").unwrap(),
            open(Subject {
                pull_request: Some(42),
                ..Default::default()
            })
        );
        assert_eq!(
            parse_line("-").unwrap(),
            open(Subject {
                stdin: true,
                ..Default::default()
            })
        );
    }

    #[test]
    fn reviews_and_explains_with_their_options() {
        assert_eq!(
            parse_line("review main..HEAD --agent codex --print").unwrap(),
            Command::Review {
                subject: Subject {
                    revision: Some("main..HEAD".into()),
                    ..Default::default()
                },
                agent: Some("codex".into()),
                print: true,
                last: false,
            }
        );
        assert_eq!(
            parse_line("review --last").unwrap(),
            Command::Review {
                subject: Subject::default(),
                agent: None,
                print: false,
                last: true,
            }
        );
        assert_eq!(
            parse_line("explain abc123 --agent=opencode --print").unwrap(),
            Command::Explain {
                revision: "abc123".into(),
                agent: Some("opencode".into()),
                print: true,
            }
        );
    }

    #[test]
    fn knows_its_setup_commands() {
        assert_eq!(parse_line("--help").unwrap(), Command::Help);
        assert_eq!(parse_line("review -h").unwrap(), Command::Help);
        assert_eq!(parse_line("doctor").unwrap(), Command::Doctor);
        assert_eq!(
            parse_line("git-install --default").unwrap(),
            Command::GitInstall {
                default: true,
                uninstall: false,
            }
        );
        assert_eq!(
            parse_line("--difftool /tmp/l /tmp/r src/a.ts").unwrap(),
            Command::Difftool {
                left: "/tmp/l".into(),
                right: "/tmp/r".into(),
                merged: Some("src/a.ts".into()),
            }
        );
    }

    #[test]
    fn refuses_what_does_not_make_sense() {
        for line in [
            "main..HEAD --staged",
            "pr 12 -- src",
            "pr twelve",
            "a b",
            "--print",
            "--bogus",
            "explain",
            "explain HEAD --staged",
            "review --last --print",
            "doctor now",
            "git-install --default --uninstall",
            "--agent",
        ] {
            assert!(parse_line(line).is_err(), "{line}");
        }
    }
}
