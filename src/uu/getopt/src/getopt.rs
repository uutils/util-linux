use clap::{Arg, ArgAction, Command, crate_version};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("getopt.md");
const USAGE: &str = help_usage!("getopt.md");

#[cfg(target_os = "linux")]
mod linux_impl {

    enum OrderingMode {
        RequireOrder,
        Permute,
        ReturnInOrder,
    }

    pub fn parse_options(
        options: &str,
        longoptions: Option<&String>,
        alternative: bool,
        mut parameters: Vec<String>,
        name: &str,
        quiet: bool,
    ) -> Result<String, i32> {
        // Determine the ordering mode based on the first character of options
        let ordering = if options.starts_with('+') {
            OrderingMode::RequireOrder
        } else if options.starts_with('-') {
            OrderingMode::ReturnInOrder
        } else if std::env::var("POSIXLY_CORRECT").is_ok() {
            OrderingMode::RequireOrder
        } else {
            OrderingMode::Permute
        };

        // Clean up the options string by removing leading + or -
        let options = if let Some(stripped) = options
            .strip_prefix('+')
            .or_else(|| options.strip_prefix('-'))
        {
            stripped
        } else {
            options
        };

        let mut result = String::new();
        let mut optind = 0; // Current index in parameters
        let mut first_nonopt = 0; // First non-option argument
        let mut last_nonopt = 0; // Last non-option argument

        // Process arguments until we're done
        while optind < parameters.len() {
            let param = parameters[optind].clone();

            // Handle non-option arguments
            if param == "--" {
                // Mark everything after -- as non-options
                result.push_str("-- ");
                optind += 1;
                break;
            } else if !param.starts_with('-') || param == "-" {
                match ordering {
                    OrderingMode::RequireOrder => {
                        // Stop processing options
                        break;
                    }
                    OrderingMode::ReturnInOrder => {
                        // Treat as special option with code 1
                        result.push_str(&format!("1 '{}' ", param));
                        optind += 1;
                        continue;
                    }
                    OrderingMode::Permute => {
                        // Mark as non-option for later permutation
                        if first_nonopt == last_nonopt {
                            first_nonopt = optind;
                        }
                        last_nonopt = optind + 1;
                        optind += 1;
                        continue;
                    }
                }
            }

            // If we're in PERMUTE mode and have accumulated non-options, permute them
            if matches!(ordering, OrderingMode::Permute)
                && first_nonopt != last_nonopt
                && last_nonopt != optind
            {
                permute_arguments(&mut parameters, first_nonopt, last_nonopt, optind);
                first_nonopt = optind - (last_nonopt - first_nonopt);
                last_nonopt = optind;
            }

            // Handle long options (GNU style: either --option or -option with alternative flag)
            if param.starts_with("--") || (alternative && param.starts_with('-') && param.len() > 2)
            {
                let mut params_iter = parameters[optind + 1..].iter().peekable();
                match parse_long_option(&param, longoptions, &mut params_iter, name, quiet) {
                    Ok(parsed) => {
                        result.push_str(&parsed);
                        // Update optind based on how many arguments were consumed
                        let consumed = parameters[optind + 1..].len() - params_iter.count();
                        optind += 1 + consumed;
                    }
                    Err(code) => {
                        return Err(code);
                    }
                }
            } else {
                // Handle short options (GNU style: supports bundled options like -abc)
                let mut params_iter = parameters[optind + 1..].iter().peekable();
                match parse_short_options(&param, options, &mut params_iter, name, quiet) {
                    Ok(parsed) => {
                        result.push_str(&parsed);
                        // Update optind based on how many arguments were consumed
                        let consumed = parameters[optind + 1..].len() - params_iter.count();
                        optind += 1 + consumed;
                    }
                    Err(code) => {
                        return Err(code);
                    }
                }
            }
        }

        // Add all remaining non-option arguments
        if optind < parameters.len() || first_nonopt != last_nonopt {
            result.push_str("-- ");
            // Add non-options that were skipped during permutation
            for param in parameters.iter().take(last_nonopt).skip(first_nonopt) {
                result.push_str(&format!("'{}' ", param));
            }
            // Add remaining arguments after optind
            for param in parameters.iter().skip(optind) {
                result.push_str(&format!("'{}' ", param));
            }
        }

        Ok(result)
    }

    // Permute arguments to move non-options after options
    fn permute_arguments(
        args: &mut [String],
        first_nonopt: usize,
        last_nonopt: usize,
        optind: usize,
    ) {
        if first_nonopt == last_nonopt || last_nonopt == optind {
            return; // Nothing to permute
        }

        // It reorders arguments to move non-options after options
        if last_nonopt - first_nonopt > optind - last_nonopt {
            // Bottom segment is shorter, swap it with the top part of the top segment
            let len = last_nonopt - first_nonopt;
            // Swap the segments
            for i in 0..len {
                args.swap(first_nonopt + i, optind - len + i);
            }
        } else {
            // Top segment is shorter, swap it with the bottom part of the bottom segment
            let len = optind - last_nonopt;
            // Swap the segments
            for i in 0..len {
                args.swap(first_nonopt + i, last_nonopt + i);
            }
        }
    }

    fn parse_long_option(
        param: &str,
        longoptions: Option<&String>,
        params_iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
        name: &str,
        quiet: bool,
    ) -> Result<String, i32> {
        if let Some(long_opts) = longoptions {
            let opt_name = if let Some(stripped) = param.strip_prefix("--") {
                stripped
            } else {
                &param[1..]
            };

            let (opt_name, opt_arg) = if let Some(idx) = opt_name.find('=') {
                (&opt_name[..idx], Some(&opt_name[idx + 1..]))
            } else {
                (opt_name, None)
            };

            // Check if the long option exists
            let long_opts_vec: Vec<&str> = long_opts.split(',').map(|s| s.trim()).collect();
            let mut found = false;
            let mut requires_arg = false;
            let mut optional_arg = false;
            let mut found_opt_spec = "";

            for long_opt in &long_opts_vec {
                let mut opt_spec = long_opt.trim();
                requires_arg = false;
                optional_arg = false;

                if opt_spec.ends_with("::") {
                    opt_spec = &opt_spec[..opt_spec.len() - 2];
                    optional_arg = true;
                } else if opt_spec.ends_with(':') {
                    opt_spec = &opt_spec[..opt_spec.len() - 1];
                    requires_arg = true;
                }

                // GNU style: allow abbreviated long options if unambiguous
                if opt_spec.starts_with(opt_name) {
                    if opt_spec == opt_name {
                        // Exact match
                        found = true;
                        found_opt_spec = opt_spec;
                        break;
                    } else if !found {
                        // Partial match
                        found = true;
                        found_opt_spec = opt_spec;
                    } else {
                        // Ambiguous abbreviation
                        if !quiet {
                            eprintln!("{}: option '--{}' is ambiguous", name, opt_name);
                        }
                        return Err(1);
                    }
                }
            }

            if !found {
                if !quiet {
                    eprintln!("{}: unrecognized option '--{}'", name, opt_name);
                }
                return Err(1);
            }

            // Use the found option spec
            let opt_name = found_opt_spec;

            if requires_arg {
                if let Some(arg_value) = opt_arg {
                    Ok(format!("--{} '{}' ", opt_name, arg_value))
                } else if let Some(next_param) = params_iter.next() {
                    Ok(format!("--{} '{}' ", opt_name, next_param))
                } else {
                    if !quiet {
                        eprintln!("{}: option '--{}' requires an argument", name, opt_name);
                    }
                    Err(1)
                }
            } else if optional_arg {
                if let Some(arg_value) = opt_arg {
                    Ok(format!("--{} '{}' ", opt_name, arg_value))
                } else {
                    Ok(format!("--{} ", opt_name))
                }
            } else {
                if opt_arg.is_some() {
                    if !quiet {
                        eprintln!(
                            "{}: option '--{}' doesn't allow an argument",
                            name, opt_name
                        );
                    }
                    return Err(1);
                }
                Ok(format!("--{} ", opt_name))
            }
        } else {
            if !quiet {
                eprintln!("{}: long options are not supported", name);
            }
            Err(1)
        }
    }

    fn parse_short_options(
        param: &str,
        options: &str,
        params_iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
        name: &str,
        quiet: bool,
    ) -> Result<String, i32> {
        let mut result = String::new();
        // Skip the leading '-'
        let mut chars = param.chars().skip(1).peekable();

        while let Some(c) = chars.next() {
            // Check if the option is valid
            let opt_pos = options.find(c);
            if opt_pos.is_none() {
                if !quiet {
                    eprintln!("{}: invalid option -- '{}'", name, c);
                }
                return Err(1);
            }

            let opt_pos = opt_pos.unwrap();

            // Check if option requires an argument
            let requires_arg =
                opt_pos + 1 < options.len() && options.chars().nth(opt_pos + 1) == Some(':');
            let optional_arg = requires_arg
                && opt_pos + 2 < options.len()
                && options.chars().nth(opt_pos + 2) == Some(':');

            if requires_arg {
                // Handle option with argument
                let arg_value = if chars.peek().is_some() {
                    // GNU style: -ovalue (remaining chars are the argument)
                    let arg: String = chars.collect();
                    Some(arg)
                } else if let Some(next_param) = params_iter.next() {
                    // Next parameter is the argument
                    Some((*next_param).clone())
                } else if optional_arg {
                    // Optional argument is missing, which is fine
                    None
                } else {
                    // Missing required argument
                    if !quiet {
                        eprintln!("{}: option requires an argument -- '{}'", name, c);
                    }
                    return Err(1);
                };

                // Add the option and its argument to the result
                if let Some(arg) = arg_value {
                    result.push_str(&format!("-{} '{}' ", c, arg));
                } else {
                    result.push_str(&format!("-{} ", c));
                }

                // After processing an option with an argument, we're done with this parameter
                break;
            } else {
                // Simple option without argument
                result.push_str(&format!("-{} ", c));
            }
        }

        Ok(result)
    }
}

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let options = matches
        .get_one::<String>("options")
        .map(|s| s.as_str())
        .unwrap_or("");
    let longoptions = matches.get_one::<String>("longoptions");
    let alternative = matches.get_flag("alternative");
    let quiet = matches.get_flag("quiet");
    let name = matches
        .get_one::<String>("name")
        .map(|s| s.as_str())
        .unwrap_or("");

    // Get the arguments to parse (everything after --)
    let parameters: Vec<String> = matches
        .get_many::<String>("parameters")
        .map_or_else(Vec::new, |values| values.cloned().collect());

    if parameters.is_empty() {
        if !quiet {
            eprintln!("{}: No parameters given", name);
        }
        process::exit(1);
    }

    // Parse the options and generate output
    match linux_impl::parse_options(options, longoptions, alternative, parameters, name, quiet) {
        Ok(result) => {
            print!("{}", result);
            Ok(())
        }
        Err(code) => {
            process::exit(code);
        }
    }
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .allow_hyphen_values(true)
        .arg(
            Arg::new("alternative")
                .short('a')
                .long("alternative")
                .help("Allow long options with a single -")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Disable error reporting by getopt")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("options")
                .short('o')
                .long("options")
                .help("Short options to be recognized")
                .required(true)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("longoptions")
                .short('l')
                .long("longoptions")
                .help("Long options to be recognized")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .help("The name that will be used by the getopt for error reporting")
                .action(ArgAction::Set)
                .default_value("getopt"),
        )
        .arg(
            Arg::new("parameters")
                .help("Parameters to be parsed")
                .action(ArgAction::Append)
                .num_args(0..)
                .trailing_var_arg(true),
        )
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    eprintln!("`getopt` is fully supported only on Linux");
    Err(uucore::error::USimpleError::new(
        1,
        "`getopt` is available only on Linux.",
    ))
}
