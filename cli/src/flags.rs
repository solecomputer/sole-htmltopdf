#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value {
    None,
    One(&'static str),
    Repeatable(&'static str),
    Two(&'static str, &'static str),
}

impl Value {
    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::None => 0,
            Self::One(_) | Self::Repeatable(_) => 1,
            Self::Two(_, _) => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Disposition {
    Implemented,

    Translated(&'static str),

    Unsupported,

    ModeDependent {
        modern: &'static str,
        emulation: &'static str,
    },

    Refused(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Flag {
    pub(crate) long: &'static str,
    pub(crate) short: Option<char>,
    pub(crate) value: Value,
    pub(crate) disposition: Disposition,
}

pub(crate) fn by_long(name: &str) -> Option<&'static Flag> {
    if !name.starts_with("--") || name.starts_with("---") {
        return None;
    }
    FLAGS.iter().find(|f| f.long == name)
}

pub(crate) fn by_short(letter: char) -> Option<&'static Flag> {
    FLAGS.iter().find(|f| f.short == Some(letter))
}

pub(crate) const FLAGS: &[Flag] = &[
    Flag {
        long: "--load-error-handling",
        short: None,
        value: Value::One("handler"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--load-media-error-handling",
        short: None,
        value: Value::One("handler"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--radiobutton-checked-svg",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--toc-level-indentation",
        short: None,
        value: Value::One("width"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--viewport-size",
        short: None,
        value: Value::One("size"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--stylesheet",
        short: None,
        value: Value::Repeatable("path"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--asset-root",
        short: None,
        value: Value::One("directory"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--quirks",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--strict",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--collate",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--no-collate",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--cookie-jar",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--copies",
        short: None,
        value: Value::One("number"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--dpi",
        short: Some('d'),
        value: Value::One("dpi"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--extended-help",
        short: Some('H'),
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--grayscale",
        short: Some('g'),
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--help",
        short: Some('h'),
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--htmldoc",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--image-dpi",
        short: None,
        value: Value::One("integer"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--image-quality",
        short: None,
        value: Value::One("integer"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--license",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--log-level",
        short: None,
        value: Value::One("level"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--lowquality",
        short: Some('l'),
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--manpage",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--margin-bottom",
        short: Some('B'),
        value: Value::One("unitreal"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--margin-left",
        short: Some('L'),
        value: Value::One("unitreal"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--margin-right",
        short: Some('R'),
        value: Value::One("unitreal"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--margin-top",
        short: Some('T'),
        value: Value::One("unitreal"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--orientation",
        short: Some('O'),
        value: Value::One("orientation"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--page-height",
        short: None,
        value: Value::One("unitreal"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--page-size",
        short: Some('s'),
        value: Value::One("Size"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--page-width",
        short: None,
        value: Value::One("unitreal"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--no-pdf-compression",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--quiet",
        short: Some('q'),
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--read-args-from-stdin",
        short: None,
        value: Value::None,
        disposition: Disposition::Refused(
            "reading arguments from stdin is not in this version, and continuing would render with arguments we were told not to use",
        ),
    },
    Flag {
        long: "--readme",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--title",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--use-xserver",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--version",
        short: Some('V'),
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--dump-default-toc-xsl",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--dump-outline",
        short: None,
        value: Value::One("file"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--outline",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--no-outline",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--outline-depth",
        short: None,
        value: Value::One("level"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--allow",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--background",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--no-background",
        short: None,
        value: Value::None,

        disposition: Disposition::Translated(
            "renders without the document's colours: no backgrounds AND no \
             text colours, because the foreground chosen against a surface \
             is unreadable without it",
        ),
    },
    Flag {
        long: "--bypass-proxy-for",
        short: None,
        value: Value::One("value"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--cache-dir",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--checkbox-checked-svg",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--checkbox-svg",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--cookie",
        short: None,
        value: Value::Two("name", "value"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--custom-header",
        short: None,
        value: Value::Two("name", "value"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--custom-header-propagation",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--no-custom-header-propagation",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--debug-javascript",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--no-debug-javascript",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--default-header",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--encoding",
        short: None,
        value: Value::One("encoding"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--disable-external-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--enable-external-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-forms",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--enable-forms",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--images",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--no-images",
        short: None,
        value: Value::None,

        disposition: Disposition::Translated(
            "renders without images, and acquires none: no file a document \
             names is read and no inline payload is decoded",
        ),
    },
    Flag {
        long: "--disable-internal-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--enable-internal-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-javascript",
        short: Some('n'),
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--enable-javascript",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--javascript-delay",
        short: None,
        value: Value::One("msec"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--keep-relative-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-local-file-access",
        short: None,
        value: Value::None,
        disposition: Disposition::Translated(
            "already the default: local file access is deny-by-default (deny-by-default policy)",
        ),
    },
    Flag {
        long: "--enable-local-file-access",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--minimum-font-size",
        short: None,
        value: Value::One("int"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--exclude-from-outline",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--include-in-outline",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--page-offset",
        short: None,
        value: Value::One("offset"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--password",
        short: None,
        value: Value::One("password"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-plugins",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--enable-plugins",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--post",
        short: None,
        value: Value::Two("name", "value"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--post-file",
        short: None,
        value: Value::Two("name", "path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--print-media-type",
        short: None,
        value: Value::None,

        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--no-print-media-type",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--proxy",
        short: Some('p'),
        value: Value::One("proxy"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--proxy-hostname-lookup",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--radiobutton-svg",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--resolve-relative-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--run-script",
        short: None,
        value: Value::One("js"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-smart-shrinking",
        short: None,
        value: Value::None,
        disposition: Disposition::ModeDependent {
            modern: "accepted and inert: the modern renderer never shrinks, \
                     so there is nothing to disable (emulation policy)",
            emulation: "smart shrinking is disabled for this render",
        },
    },
    Flag {
        long: "--enable-smart-shrinking",
        short: None,
        value: Value::None,
        disposition: Disposition::ModeDependent {
            modern: "accepted and inert: smart shrinking is a wkhtmltopdf \
                     0.12.6 quirk, reachable only with --quirks (emulation policy)",
            emulation: "smart shrinking is on, which is the bundle's default",
        },
    },
    Flag {
        long: "--ssl-crt-path",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--ssl-key-password",
        short: None,
        value: Value::One("password"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--ssl-key-path",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--stop-slow-scripts",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--no-stop-slow-scripts",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-toc-back-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--enable-toc-back-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--user-style-sheet",
        short: None,
        value: Value::One("path"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--username",
        short: None,
        value: Value::One("username"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--window-status",
        short: None,
        value: Value::One("windowStatus"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--zoom",
        short: None,
        value: Value::One("float"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--footer-center",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--footer-font-name",
        short: None,
        value: Value::One("name"),
        disposition: Disposition::Translated(
            "the PDF base 14 are used as written; any other face falls back to \
             Helvetica and is named on stderr, never substituted silently",
        ),
    },
    Flag {
        long: "--footer-font-size",
        short: None,
        value: Value::One("size"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--footer-html",
        short: None,
        value: Value::One("url"),
        disposition: Disposition::Translated(
            "the markup is drawn as text in the margin the invocation already \
             declared: images, backgrounds, borders and table geometry inside \
             a band are not drawn in this version, and the band never pushes \
             the body down, so adding a footer cannot repaginate a document",
        ),
    },
    Flag {
        long: "--footer-left",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--footer-line",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--no-footer-line",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--footer-right",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--footer-spacing",
        short: None,
        value: Value::One("real"),
        disposition: Disposition::Translated(
            "spacing is taken from WITHIN the declared margin rather than pushed \
             into the body: enabling a header cannot repaginate a document",
        ),
    },
    Flag {
        long: "--header-center",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--header-font-name",
        short: None,
        value: Value::One("name"),
        disposition: Disposition::Translated(
            "the PDF base 14 are used as written; any other face falls back to \
             Helvetica and is named on stderr, never substituted silently",
        ),
    },
    Flag {
        long: "--header-font-size",
        short: None,
        value: Value::One("size"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--header-html",
        short: None,
        value: Value::One("url"),
        disposition: Disposition::Translated(
            "the markup is drawn as text in the margin the invocation already \
             declared: images, backgrounds, borders and table geometry inside \
             a band are not drawn in this version, and the band never pushes \
             the body down, so adding a header cannot repaginate a document",
        ),
    },
    Flag {
        long: "--header-left",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--header-line",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--no-header-line",
        short: None,
        value: Value::None,
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--header-right",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--header-spacing",
        short: None,
        value: Value::One("real"),
        disposition: Disposition::Translated(
            "spacing is taken from WITHIN the declared margin rather than pushed \
             into the body: enabling a header cannot repaginate a document",
        ),
    },
    Flag {
        long: "--replace",
        short: None,
        value: Value::Two("name", "value"),
        disposition: Disposition::Implemented,
    },
    Flag {
        long: "--disable-dotted-lines",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--toc-header-text",
        short: None,
        value: Value::One("text"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--disable-toc-links",
        short: None,
        value: Value::None,
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--toc-text-size-shrink",
        short: None,
        value: Value::One("real"),
        disposition: Disposition::Unsupported,
    },
    Flag {
        long: "--xsl-style-sheet",
        short: None,
        value: Value::One("file"),
        disposition: Disposition::Unsupported,
    },
];
