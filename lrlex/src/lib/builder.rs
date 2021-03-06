//! Build grammars at run-time.

use std::{
    collections::{HashMap, HashSet},
    convert::AsRef,
    env::{current_dir, var},
    error::Error,
    fmt::Debug,
    fs::{self, create_dir_all, read_to_string, File},
    hash::Hash,
    io::Write,
    path::{Path, PathBuf}
};

use lazy_static::lazy_static;
use num_traits::{PrimInt, Unsigned};
use regex::Regex;
use try_from::TryFrom;
use typename::TypeName;

use crate::{lexer::LexerDef, parser::parse_lex};

const RUST_FILE_EXT: &str = "rs";

lazy_static! {
    static ref RE_TOKEN_ID: Regex = Regex::new(r"^[a-zA-Z_][a-zA-Z_0-9]*$").unwrap();
}

/// A `LexerBuilder` allows one to specify the criteria for building a statically generated
/// lexer.
pub struct LexerBuilder<StorageT = u32> {
    rule_ids_map: Option<HashMap<String, StorageT>>,
    allow_missing_terms_in_lexer: bool,
    allow_missing_tokens_in_parser: bool
}

impl<StorageT> LexerBuilder<StorageT>
where
    StorageT: Copy + Debug + Eq + Hash + PrimInt + TryFrom<usize> + TypeName + Unsigned
{
    /// Create a new `LexerBuilder`.
    ///
    /// `StorageT` must be an unsigned integer type (e.g. `u8`, `u16`) which is big enough to index
    /// all the tokens, rules, and productions in the lexer and less than or equal in size
    /// to `usize` (e.g. on a 64-bit machine `u128` would be too big). If you are lexing large
    /// files, the additional storage requirements of larger integer types can be noticeable, and
    /// in such cases it can be worth specifying a smaller type. `StorageT` defaults to `u32` if
    /// unspecified.
    ///
    /// # Examples
    ///
    /// ```text
    /// LexerBuilder::<u8>::new()
    ///     .process_file_in_src("grm.l", None)
    ///     .unwrap();
    /// ```
    pub fn new() -> Self {
        LexerBuilder {
            rule_ids_map: None,
            allow_missing_terms_in_lexer: false,
            allow_missing_tokens_in_parser: true
        }
    }

    /// Set this lexer builder's map of rule IDs to `rule_ids_map`. By default, lexing rules have
    /// arbitrary, but distinct, IDs. Setting the map of rule IDs (from rule names to `StorageT`)
    /// allows users to synchronise a lexer and parser and to check that all rules are used by both
    /// parts).
    pub fn rule_ids_map(mut self, rule_ids_map: HashMap<String, StorageT>) -> Self {
        self.rule_ids_map = Some(rule_ids_map);
        self
    }

    /// Given the filename `a/b.l` as input, statically compile the file `src/a/b.l` into a Rust
    /// module which can then be imported using `lrlex_mod!("a/b.l")`. This is a convenience
    /// function around [`process_file`](struct.LexerBuilder.html#method.process_file) which makes
    /// it easier to compile `.l` files stored in a project's `src/` directory.
    pub fn process_file_in_src(
        self,
        srcp: &str
    ) -> Result<(Option<HashSet<String>>, Option<HashSet<String>>), Box<dyn Error>> {
        let mut inp = current_dir()?;
        inp.push("src");
        inp.push(srcp);
        let mut outp = PathBuf::new();
        outp.push(var("OUT_DIR").unwrap());
        outp.push(Path::new(srcp).parent().unwrap().to_str().unwrap());
        create_dir_all(&outp)?;
        let mut leaf = Path::new(srcp)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        leaf.push_str(&format!(".{}", RUST_FILE_EXT));
        outp.push(leaf);
        self.process_file(inp, outp)
    }

    /// Statically compile the `.l` file `inp` into Rust, placing the output into the file `outp`.
    /// The latter defines a module with a function `lexerdef()`, which returns a
    /// [`LexerDef`](struct.LexerDef.html) that can then be used as normal.
    pub fn process_file<P, Q>(
        self,
        inp: P,
        outp: Q
    ) -> Result<(Option<HashSet<String>>, Option<HashSet<String>>), Box<dyn Error>>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>
    {
        let inc = read_to_string(&inp).unwrap();
        let mut lexerdef = parse_lex::<StorageT>(&inc)?;
        let (missing_from_lexer, missing_from_parser) = match self.rule_ids_map {
            Some(ref rim) => {
                // Convert from HashMap<String, _> to HashMap<&str, _>
                let owned_map = rim
                    .iter()
                    .map(|(x, y)| (&**x, *y))
                    .collect::<HashMap<_, _>>();
                match lexerdef.set_rule_ids(&owned_map) {
                    (x, y) => (
                        x.map(|a| a.iter().map(|&b| b.to_string()).collect::<HashSet<_>>()),
                        y.map(|a| a.iter().map(|&b| b.to_string()).collect::<HashSet<_>>())
                    )
                }
            }
            None => (None, None)
        };

        if !self.allow_missing_terms_in_lexer {
            if let Some(ref mfl) = missing_from_lexer {
                eprintln!("Error: the following tokens are used in the grammar but are not defined in the lexer:");
                for n in mfl {
                    eprintln!("    {}", n);
                }
                fs::remove_file(&outp).ok();
                panic!();
            }
        }
        if !self.allow_missing_tokens_in_parser {
            if let Some(ref mfp) = missing_from_parser {
                eprintln!("Error: the following tokens are defined in the lexer but not used in the grammar:");
                for n in mfp {
                    eprintln!("    {}", n);
                }
                fs::remove_file(&outp).ok();
                panic!();
            }
        }

        let mut outs = String::new();
        // At this point we potentially have a filename a.l.rs, so strip off all the extensions.
        let mut mod_name = inp.as_ref().to_str().unwrap();
        loop {
            let mod_stem = Path::new(mod_name).file_stem().unwrap().to_str().unwrap();
            if mod_name == mod_stem {
                break;
            }
            mod_name = mod_stem;
        }
        // Header
        outs.push_str(&format!("mod {}_l {{", mod_name));
        lexerdef.rust_pp(&mut outs);

        // Token IDs
        if let Some(ref rim) = self.rule_ids_map {
            for (n, id) in rim {
                if RE_TOKEN_ID.is_match(n) {
                    outs.push_str(&format!(
                        "#[allow(dead_code)]\npub const T_{}: {} = {:?};\n",
                        n.to_ascii_uppercase(),
                        StorageT::type_name(),
                        *id
                    ));
                }
            }
        }

        // Footer
        outs.push_str("}");

        // If the file we're about to write out already exists with the same contents, then we
        // don't overwrite it (since that will force a recompile of the file, and relinking of the
        // binary etc).
        if let Ok(curs) = read_to_string(&outp) {
            if curs == outs {
                return Ok((missing_from_lexer, missing_from_parser));
            }
        }
        let mut f = File::create(outp)?;
        f.write_all(outs.as_bytes())?;
        Ok((missing_from_lexer, missing_from_parser))
    }

    /// If passed false, tokens used in the grammar but not defined in the lexer will cause a
    /// panic at lexer generation time. Defaults to false.
    pub fn allow_missing_terms_in_lexer(mut self, allow: bool) -> Self {
        self.allow_missing_terms_in_lexer = allow;
        self
    }

    /// If passed false, tokens defined in the lexer but not used in the grammar will cause a
    /// panic at lexer generation time. Defaults to true (since lexers sometimes define tokens such
    /// as reserved words, which are intentionally not in the grammar).
    pub fn allow_missing_tokens_in_parser(mut self, allow: bool) -> Self {
        self.allow_missing_tokens_in_parser = allow;
        self
    }
}

impl<StorageT: Copy + Debug + Eq + TypeName> LexerDef<StorageT> {
    pub(crate) fn rust_pp(&self, outs: &mut String) {
        // Header
        outs.push_str(&format!(
            "use lrlex::{{LexerDef, Rule}};

#[allow(dead_code)]
pub fn lexerdef() -> LexerDef<{}> {{
    let rules = vec![",
            StorageT::type_name()
        ));

        // Individual rules
        for r in &self.rules {
            let tok_id = match r.tok_id {
                Some(ref t) => format!("Some({:?})", t),
                None => "None".to_owned()
            };
            let n = match r.name {
                Some(ref n) => format!("Some({:?}.to_string())", n),
                None => "None".to_owned()
            };
            outs.push_str(&format!(
                "
Rule::new({}, {}, \"{}\".to_string()).unwrap(),",
                tok_id,
                n,
                r.re_str.replace("\\", "\\\\").replace("\"", "\\\"")
            ));
        }

        // Footer
        outs.push_str(
            "
];
    LexerDef::new(rules)
}
"
        );
    }
}
