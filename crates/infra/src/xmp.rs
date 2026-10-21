//! XMP sidecars: writing culling decisions and reading them back
//! (ADR-0009).
//!
//! Properties written, for the tools that read them:
//!
//! | Property | Value | Read by |
//! |---|---|---|
//! | `xmp:Rating` | stars 0–5, or −1 when rejected | darktable, Lightroom, Bridge |
//! | `digiKam:PickLabel` | 3 pick, 1 reject, 0 none | digiKam |
//! | `picturesque:Flag`, `picturesque:Stars` | flag and stars | Picturesque |
//! | `darktable:colorlabels` | colour labels, 0 red … 4 purple | darktable |
//! | `xmp:Label` | `Red` … `Purple` | Lightroom, Bridge |
//! | `digiKam:ColorLabel` | 1 red, 3 yellow, 4 green, 5 blue, 6 purple | digiKam |
//! | `picturesque:Label` | the label Picturesque set | Picturesque |
//!
//! Colour labels belong to the user as much as to Picturesque: only the
//! label Picturesque set last (`picturesque:Label`) is ever removed or
//! replaced, so a label set in another tool stays.
//!
//! Existing sidecars are rewritten as an event stream: only these
//! properties (matched by namespace URI, whatever their prefix, in
//! attribute or element form) are replaced; everything else is copied.

use std::collections::HashMap;
use std::io::Cursor;

use picturesque_app::ports::sidecars::{ColorLabel, SidecarValues};
use picturesque_core::decision::{Decision, Flag, Rating};
use quick_xml::Reader;
use quick_xml::Writer;
use quick_xml::events::{BytesStart, Event};

pub const NS_RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const NS_XMP: &str = "http://ns.adobe.com/xap/1.0/";
pub const NS_DIGIKAM: &str = "http://www.digikam.org/ns/1.0/";
pub const NS_PICTURESQUE: &str = "https://github.com/denyszorinets/picturesque/ns/1.0/";
pub const NS_DARKTABLE: &str = "http://darktable.sf.net/";

/// Simple properties this module owns: (namespace, local name), in the
/// order they are written.
const OWNED: &[(&str, &str)] = &[
    (NS_XMP, "Rating"),
    (NS_DIGIKAM, "PickLabel"),
    (NS_PICTURESQUE, "Flag"),
    (NS_PICTURESQUE, "Stars"),
    (NS_XMP, "Label"),
    (NS_DIGIKAM, "ColorLabel"),
    (NS_PICTURESQUE, "Label"),
];

/// darktable's list of colour labels, rewritten as an element.
const COLORLABELS: (&str, &str) = (NS_DARKTABLE, "colorlabels");

/// Preferred prefixes for the namespaces we add.
const PREFIXES: &[(&str, &str)] = &[
    (NS_XMP, "xmp"),
    (NS_DIGIKAM, "digiKam"),
    (NS_PICTURESQUE, "picturesque"),
    (NS_DARKTABLE, "darktable"),
];

fn darktable_label(label: ColorLabel) -> u8 {
    match label {
        ColorLabel::Red => 0,
        ColorLabel::Yellow => 1,
        ColorLabel::Green => 2,
        ColorLabel::Blue => 3,
        ColorLabel::Purple => 4,
    }
}

fn lightroom_label(label: ColorLabel) -> &'static str {
    match label {
        ColorLabel::Red => "Red",
        ColorLabel::Yellow => "Yellow",
        ColorLabel::Green => "Green",
        ColorLabel::Blue => "Blue",
        ColorLabel::Purple => "Purple",
    }
}

fn digikam_label(label: ColorLabel) -> &'static str {
    match label {
        ColorLabel::Red => "1",
        ColorLabel::Yellow => "3",
        ColorLabel::Green => "4",
        ColorLabel::Blue => "5",
        ColorLabel::Purple => "6",
    }
}

/// The colour labels a sidecar holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Labels {
    /// The label Picturesque set (`picturesque:Label`).
    pub ours: Option<ColorLabel>,
    /// darktable's labels, in file order.
    pub darktable: Vec<u8>,
    pub lightroom: Option<String>,
    pub digikam: Option<String>,
}

impl Labels {
    /// The labels after Picturesque sets `label` (or removes its own).
    fn with(&self, label: Option<ColorLabel>) -> Labels {
        let mut darktable = self.darktable.clone();
        if let Some(old) = self.ours {
            darktable.retain(|&l| l != darktable_label(old));
        }
        if let Some(new) = label
            && !darktable.contains(&darktable_label(new))
        {
            darktable.push(darktable_label(new));
        }
        // Single-valued fields: ours replaces, and goes only if still ours.
        let single = |current: &Option<String>, name: fn(ColorLabel) -> &'static str| match label {
            Some(new) => Some(name(new).to_owned()),
            None if self.ours.map(name) == current.as_deref() => None,
            None => current.clone(),
        };
        Labels {
            ours: label,
            darktable,
            lightroom: single(&self.lightroom, lightroom_label),
            digikam: single(&self.digikam, digikam_label),
        }
    }
}

/// A sidecar could not be read or merged; it is left untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmpError(pub String);

impl std::fmt::Display for XmpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XMP sidecar: {}", self.0)
    }
}

impl std::error::Error for XmpError {}

fn err(e: impl std::fmt::Display) -> XmpError {
    XmpError(e.to_string())
}

fn owned_values(decision: Decision, labels: &Labels) -> Vec<(&'static str, &'static str, String)> {
    let stars = decision.rating.stars();
    let rating = if decision.flag == Flag::Reject {
        "-1".to_owned()
    } else {
        stars.to_string()
    };
    let pick = match decision.flag {
        Flag::Pick => "3",
        Flag::Reject => "1",
        Flag::Unflagged => "0",
    };
    let mut values = vec![
        (NS_XMP, "Rating", rating),
        (NS_DIGIKAM, "PickLabel", pick.to_owned()),
        (NS_PICTURESQUE, "Flag", decision.flag.as_str().to_owned()),
        (NS_PICTURESQUE, "Stars", stars.to_string()),
    ];
    if let Some(label) = &labels.lightroom {
        values.push((NS_XMP, "Label", label.clone()));
    }
    if let Some(label) = &labels.digikam {
        values.push((NS_DIGIKAM, "ColorLabel", label.clone()));
    }
    if let Some(label) = labels.ours {
        values.push((NS_PICTURESQUE, "Label", label.as_str().to_owned()));
    }
    values
}

/// darktable's `colorlabels` element, written right inside the
/// `rdf:Description`.
fn colorlabels_events(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    darktable: &str,
    rdf: &str,
    labels: &[u8],
) -> Result<(), XmpError> {
    let tag = format!("{darktable}:colorlabels");
    let seq = format!("{rdf}:Seq");
    let li = format!("{rdf}:li");
    writer
        .write_event(Event::Start(BytesStart::new(tag.as_str())))
        .map_err(err)?;
    writer
        .write_event(Event::Start(BytesStart::new(seq.as_str())))
        .map_err(err)?;
    for label in labels {
        writer
            .write_event(Event::Start(BytesStart::new(li.as_str())))
            .map_err(err)?;
        writer
            .write_event(Event::Text(quick_xml::events::BytesText::new(
                &label.to_string(),
            )))
            .map_err(err)?;
        writer
            .write_event(Event::End(quick_xml::events::BytesEnd::new(li.as_str())))
            .map_err(err)?;
    }
    writer
        .write_event(Event::End(quick_xml::events::BytesEnd::new(seq.as_str())))
        .map_err(err)?;
    writer
        .write_event(Event::End(quick_xml::events::BytesEnd::new(tag.as_str())))
        .map_err(err)?;
    Ok(())
}

/// A new sidecar holding `decision`.
pub fn new_sidecar(decision: Decision) -> String {
    new_sidecar_with(SidecarValues {
        decision,
        label: None,
    })
}

/// A new sidecar holding `values`.
pub fn new_sidecar_with(values: SidecarValues) -> String {
    let labels = Labels::default().with(values.label);
    let attrs: String = owned_values(values.decision, &labels)
        .iter()
        .map(|(ns, name, value)| format!("\n    {}:{name}=\"{value}\"", prefix_for(ns)))
        .collect();
    let darktable = if labels.darktable.is_empty() {
        "/>".to_owned()
    } else {
        let items: String = labels
            .darktable
            .iter()
            .map(|l| format!("<rdf:li>{l}</rdf:li>"))
            .collect();
        format!(
            ">\n   <darktable:colorlabels><rdf:Seq>{items}</rdf:Seq></darktable:colorlabels>\n  </rdf:Description>"
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Picturesque">
 <rdf:RDF xmlns:rdf="{NS_RDF}">
  <rdf:Description rdf:about=""
    xmlns:xmp="{NS_XMP}"
    xmlns:digiKam="{NS_DIGIKAM}"
    xmlns:picturesque="{NS_PICTURESQUE}"
    xmlns:darktable="{NS_DARKTABLE}"{attrs}{darktable}
 </rdf:RDF>
</x:xmpmeta>
"#
    )
}

fn prefix_for(ns: &str) -> &'static str {
    PREFIXES
        .iter()
        .find(|(n, _)| *n == ns)
        .map(|(_, p)| *p)
        .expect("known namespace")
}

/// Namespace scopes while streaming: each element pushes its bindings.
#[derive(Default)]
struct Scopes(Vec<HashMap<String, String>>);

impl Scopes {
    fn push(&mut self, start: &BytesStart<'_>) -> Result<(), XmpError> {
        let mut scope = HashMap::new();
        for attr in start.attributes() {
            let attr = attr.map_err(err)?;
            let key = attr.key.as_ref();
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(err)?
                .into_owned();
            if key == "xmlns" {
                scope.insert(String::new(), value);
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                scope.insert(prefix.to_owned(), value);
            }
        }
        self.0.push(scope);
        Ok(())
    }

    fn pop(&mut self) {
        self.0.pop();
    }

    fn lookup(&self, prefix: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find_map(|s| s.get(prefix))
            .map(String::as_str)
    }

    /// Resolves `prefix:local` (attributes without a prefix have no namespace).
    fn resolve<'n>(&self, qname: &'n str) -> Option<(&str, &'n str)> {
        let (prefix, local) = qname.split_once(':')?;
        Some((self.lookup(prefix)?, local))
    }

    /// A prefix bound to `ns` in scope, if any.
    fn prefix_of(&self, ns: &str) -> Option<String> {
        let mut seen = std::collections::HashSet::new();
        for scope in self.0.iter().rev() {
            for (prefix, uri) in scope {
                if seen.insert(prefix.clone()) && uri == ns && !prefix.is_empty() {
                    return Some(prefix.clone());
                }
            }
        }
        None
    }
}

fn is_owned(ns: &str, local: &str) -> bool {
    OWNED.iter().any(|(n, l)| *n == ns && *l == local) || (ns, local) == COLORLABELS
}

fn is_description(scopes: &Scopes, name: &str) -> bool {
    scopes.resolve(name) == Some((NS_RDF, "Description"))
}

/// Rewrites `start` (an `rdf:Description`) without our attributes and, if
/// `add` is set, with our current values (declaring namespaces as needed).
/// Also returns the prefix for darktable's colour labels when they are to
/// be written inside it.
fn rewrite_description(
    start: &BytesStart<'_>,
    scopes: &Scopes,
    add: Option<(Decision, &Labels)>,
) -> Result<(BytesStart<'static>, Option<String>), XmpError> {
    let name = start.name().as_ref().to_owned();
    let mut out = BytesStart::new(name);
    for attr in start.attributes() {
        let attr = attr.map_err(err)?;
        let key = attr.key.as_ref();
        if let Some((ns, local)) = scopes.resolve(key)
            && is_owned(ns, local)
        {
            continue;
        }
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(err)?;
        out.push_attribute((key, value.as_ref()));
    }
    let mut darktable = None;
    if let Some((decision, labels)) = add {
        // Declarations first, then values, so merging again gives identical
        // output. Each namespace is declared at most once.
        let mut bound: Vec<(String, &str)> = Vec::new();
        let mut values = Vec::new();
        let mut namespaces: Vec<(&str, &str, String)> = owned_values(decision, labels);
        if !labels.darktable.is_empty() {
            // Only its namespace is needed here; the element follows.
            namespaces.push((NS_DARKTABLE, "colorlabels", String::new()));
        }
        for (ns, local, value) in namespaces {
            let existing = bound.iter().find(|(_, n)| *n == ns).map(|(p, _)| p.clone());
            let prefix = match existing.or_else(|| scopes.prefix_of(ns)) {
                Some(p) => p,
                None => {
                    // Our preferred prefix, or a numbered one if it is taken.
                    let base = prefix_for(ns);
                    let mut prefix = base.to_owned();
                    let mut n = 1;
                    while scopes.lookup(&prefix).is_some()
                        || bound.iter().any(|(p, _)| *p == prefix)
                    {
                        prefix = format!("{base}{n}");
                        n += 1;
                    }
                    out.push_attribute((format!("xmlns:{prefix}").as_str(), ns));
                    bound.push((prefix.clone(), ns));
                    prefix
                }
            };
            if (ns, local) == COLORLABELS {
                darktable = Some(prefix);
            } else {
                values.push((format!("{prefix}:{local}"), value));
            }
        }
        for (key, value) in &values {
            out.push_attribute((key.as_str(), value.as_str()));
        }
    }
    Ok((out, darktable))
}

/// `existing` with our properties set to `decision`, everything else kept.
/// Fails (and the caller leaves the file alone) if the XML cannot be
/// parsed or has no `rdf:Description`.
pub fn merge(existing: &str, decision: Decision) -> Result<String, XmpError> {
    merge_values(
        existing,
        SidecarValues {
            decision,
            label: None,
        },
    )
}

/// `existing` with our properties set to `values`, everything else kept
/// (see the module docs for colour labels).
pub fn merge_values(existing: &str, values: SidecarValues) -> Result<String, XmpError> {
    let labels = read_labels(existing)?.with(values.label);
    let decision = values.decision;
    let mut reader = Reader::from_str(existing);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut scopes = Scopes::default();
    let mut added = false;
    // Depth of an owned property element being skipped (element form).
    let mut skipping: usize = 0;
    // Element depth inside the current rdf:Description, to spot its properties.
    let mut in_description: Option<usize> = None;
    let mut depth: usize = 0;

    loop {
        let event = reader.read_event().map_err(err)?;
        match event {
            Event::Eof => break,
            Event::Start(start) => {
                scopes.push(&start)?;
                depth += 1;
                if skipping > 0 {
                    skipping += 1;
                    continue;
                }
                let name = start.name().as_ref().to_owned();
                if in_description == Some(depth - 1)
                    && let Some((ns, local)) = scopes.resolve(&name)
                    && is_owned(ns, local)
                {
                    skipping = 1;
                    continue;
                }
                if is_description(&scopes, &name) {
                    let (rewritten, darktable) = rewrite_description(
                        &start,
                        &scopes,
                        (!added).then_some((decision, &labels)),
                    )?;
                    added = true;
                    in_description = Some(depth);
                    writer.write_event(Event::Start(rewritten)).map_err(err)?;
                    if let Some(darktable) = darktable {
                        let rdf = scopes.prefix_of(NS_RDF).unwrap_or_else(|| "rdf".into());
                        colorlabels_events(&mut writer, &darktable, &rdf, &labels.darktable)?;
                    }
                } else {
                    writer.write_event(Event::Start(start)).map_err(err)?;
                }
            }
            Event::Empty(start) => {
                scopes.push(&start)?;
                let name = start.name().as_ref().to_owned();
                let owned_child = in_description == Some(depth)
                    && scopes
                        .resolve(&name)
                        .is_some_and(|(ns, local)| is_owned(ns, local));
                if skipping == 0 && !owned_child {
                    if is_description(&scopes, &name) {
                        let (rewritten, darktable) = rewrite_description(
                            &start,
                            &scopes,
                            (!added).then_some((decision, &labels)),
                        )?;
                        added = true;
                        match darktable {
                            // The labels go inside: open and close it.
                            Some(darktable) => {
                                let end = rewritten.to_end().into_owned();
                                writer.write_event(Event::Start(rewritten)).map_err(err)?;
                                let rdf = scopes.prefix_of(NS_RDF).unwrap_or_else(|| "rdf".into());
                                colorlabels_events(
                                    &mut writer,
                                    &darktable,
                                    &rdf,
                                    &labels.darktable,
                                )?;
                                writer.write_event(Event::End(end)).map_err(err)?;
                            }
                            None => writer.write_event(Event::Empty(rewritten)).map_err(err)?,
                        }
                    } else {
                        writer.write_event(Event::Empty(start)).map_err(err)?;
                    }
                }
                scopes.pop();
            }
            Event::End(end) => {
                if skipping > 0 {
                    skipping -= 1;
                } else {
                    if in_description == Some(depth) {
                        in_description = None;
                    }
                    writer.write_event(Event::End(end)).map_err(err)?;
                }
                scopes.pop();
                depth -= 1;
            }
            other => {
                if skipping == 0 {
                    writer.write_event(other).map_err(err)?;
                }
            }
        }
    }
    if !added {
        return Err(XmpError("no rdf:Description element".into()));
    }
    String::from_utf8(writer.into_inner().into_inner()).map_err(err)
}

/// The colour labels in a sidecar written by any tool.
pub fn read_labels(text: &str) -> Result<Labels, XmpError> {
    let mut reader = Reader::from_str(text);
    let mut scopes = Scopes::default();
    let mut labels = Labels::default();
    // Resolved names of the open elements.
    let mut open: Vec<Option<(String, String)>> = Vec::new();

    let attributes = |scopes: &Scopes, start: &BytesStart<'_>, labels: &mut Labels| {
        for attr in start.attributes() {
            let attr = attr.map_err(err)?;
            let Some((ns, local)) = scopes.resolve(attr.key.as_ref()) else {
                continue;
            };
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(err)?
                .into_owned();
            set_label(labels, ns, local, value);
        }
        Ok::<(), XmpError>(())
    };

    loop {
        match reader.read_event().map_err(err)? {
            Event::Eof => break,
            Event::Start(start) => {
                scopes.push(&start)?;
                attributes(&scopes, &start, &mut labels)?;
                let qname = start.name();
                open.push(
                    scopes
                        .resolve(qname.as_ref())
                        .map(|(ns, local)| (ns.to_owned(), local.to_owned())),
                );
            }
            Event::Empty(start) => {
                scopes.push(&start)?;
                attributes(&scopes, &start, &mut labels)?;
                scopes.pop();
            }
            Event::Text(text) => {
                let value = text.xml10_content().trim().to_owned();
                if value.is_empty() {
                    continue;
                }
                let in_colorlabels = open.iter().any(|e| {
                    e.as_ref()
                        .is_some_and(|(ns, local)| (ns.as_str(), local.as_str()) == COLORLABELS)
                });
                match open.last() {
                    Some(Some((ns, local))) if in_colorlabels && ns == NS_RDF && local == "li" => {
                        if let Ok(label) = value.parse() {
                            labels.darktable.push(label);
                        }
                    }
                    Some(Some((ns, local))) => {
                        let (ns, local) = (ns.clone(), local.clone());
                        set_label(&mut labels, &ns, &local, value);
                    }
                    _ => {}
                }
            }
            Event::End(_) => {
                open.pop();
                scopes.pop();
            }
            _ => {}
        }
    }
    Ok(labels)
}

fn set_label(labels: &mut Labels, ns: &str, local: &str, value: String) {
    match (ns, local) {
        (NS_XMP, "Label") => labels.lightroom = Some(value),
        (NS_DIGIKAM, "ColorLabel") => labels.digikam = Some(value),
        (NS_PICTURESQUE, "Label") => labels.ours = ColorLabel::from_name(&value),
        _ => {}
    }
}

/// A decision read from a sidecar written by any tool. `None` when the
/// sidecar records none of the properties we understand.
pub fn read_decision(text: &str) -> Result<Option<Decision>, XmpError> {
    let mut reader = Reader::from_str(text);
    let mut scopes = Scopes::default();
    let mut values: HashMap<(&'static str, &'static str), String> = HashMap::new();
    let mut current: Option<(&'static str, &'static str)> = None;

    let record = |scopes: &Scopes,
                  start: &BytesStart<'_>,
                  values: &mut HashMap<_, _>|
     -> Result<(), XmpError> {
        for attr in start.attributes() {
            let attr = attr.map_err(err)?;
            let key = attr.key.as_ref();
            if let Some((ns, local)) = scopes.resolve(key)
                && let Some(owned) = OWNED.iter().find(|(n, l)| *n == ns && *l == local)
            {
                values.insert(
                    *owned,
                    attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(err)?
                        .into_owned(),
                );
            }
        }
        Ok(())
    };

    loop {
        match reader.read_event().map_err(err)? {
            Event::Eof => break,
            Event::Start(start) => {
                scopes.push(&start)?;
                record(&scopes, &start, &mut values)?;
                let qname = start.name();
                let name = qname.as_ref();
                current = scopes.resolve(name).and_then(|(ns, local)| {
                    OWNED.iter().find(|(n, l)| *n == ns && *l == local).copied()
                });
            }
            Event::Empty(start) => {
                scopes.push(&start)?;
                record(&scopes, &start, &mut values)?;
                scopes.pop();
            }
            Event::Text(text) => {
                if let Some(owned) = current {
                    values.insert(owned, text.xml10_content().trim().to_owned());
                }
            }
            Event::End(_) => {
                current = None;
                scopes.pop();
            }
            _ => {}
        }
    }

    let get = |ns, local| values.get(&(ns, local)).map(String::as_str);
    // Our own properties are exact; otherwise interpret the others' fields.
    if let (Some(flag), Some(stars)) = (get(NS_PICTURESQUE, "Flag"), get(NS_PICTURESQUE, "Stars"))
        && let (Some(flag), Ok(stars)) = (Flag::from_name(flag), stars.parse::<u8>())
        && let Some(rating) = Rating::new(stars)
    {
        return Ok(Some(Decision { flag, rating }));
    }
    let rating: Option<i32> = get(NS_XMP, "Rating")
        .and_then(|r| r.parse::<f64>().ok())
        .map(|r| r.round() as i32);
    let pick: Option<i32> = get(NS_DIGIKAM, "PickLabel").and_then(|p| p.parse().ok());
    if rating.is_none() && pick.is_none() {
        return Ok(None);
    }
    let flag = match (rating, pick) {
        (Some(r), _) if r < 0 => Flag::Reject,
        (_, Some(1)) => Flag::Reject,
        (_, Some(3)) => Flag::Pick,
        _ => Flag::Unflagged,
    };
    let stars = rating.filter(|r| *r > 0).map_or(0, |r| r.min(5) as u8);
    Ok(Some(Decision {
        flag,
        rating: Rating::new(stars).unwrap_or_default(),
    }))
}

/// [`SidecarStore`] on the file system.
///
/// [`SidecarStore`]: picturesque_app::ports::sidecars::SidecarStore
#[derive(Debug, Default, Clone, Copy)]
pub struct XmpSidecars;

impl picturesque_app::ports::sidecars::SidecarStore for XmpSidecars {
    fn read(&self, path: &std::path::Path) -> Result<Option<Decision>, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        read_decision(&text).map_err(|e| e.to_string())
    }

    fn write(
        &self,
        path: &std::path::Path,
        values: SidecarValues,
    ) -> Result<(picturesque_core::hash::ContentHash, u64), String> {
        let xml = match std::fs::read_to_string(path) {
            Ok(existing) => merge_values(&existing, values).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => new_sidecar_with(values),
            Err(e) => return Err(e.to_string()),
        };
        // Write beside the target and rename over it, so a crash never
        // leaves a half-written sidecar.
        let dir = path.parent().ok_or("sidecar path has no folder")?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut tmp, xml.as_bytes()).map_err(|e| e.to_string())?;
        tmp.as_file().sync_all().map_err(|e| e.to_string())?;
        tmp.persist(path).map_err(|e| e.error.to_string())?;
        let hash = picturesque_core::hash::ContentHash::from_bytes(
            *blake3::hash(xml.as_bytes()).as_bytes(),
        );
        Ok((hash, xml.len() as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(flag: Flag, stars: u8) -> Decision {
        Decision {
            flag,
            rating: Rating::new(stars).unwrap(),
        }
    }

    #[test]
    fn new_sidecars_round_trip_every_decision() {
        for flag in [Flag::Unflagged, Flag::Pick, Flag::Reject] {
            for stars in 0..=5 {
                let xml = new_sidecar(d(flag, stars));
                assert_eq!(
                    read_decision(&xml).unwrap(),
                    Some(d(flag, stars)),
                    "{flag:?} {stars}\n{xml}"
                );
            }
        }
    }

    #[test]
    fn new_sidecars_use_the_fields_other_tools_read() {
        let xml = new_sidecar(d(Flag::Reject, 4));

        assert!(xml.contains(r#"xmp:Rating="-1""#), "{xml}");
        assert!(xml.contains(r#"digiKam:PickLabel="1""#));
        assert!(xml.contains(r#"picturesque:Stars="4""#));
        assert!(new_sidecar(d(Flag::Pick, 2)).contains(r#"xmp:Rating="2""#));
    }

    /// A darktable-style sidecar with other tools' data in both forms.
    const DARKTABLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:darktable="http://darktable.sf.net/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmp:Rating="2"
    darktable:xmp_version="5"
    darktable:history_end="3">
   <dc:subject>
    <rdf:Bag>
     <rdf:li>holiday</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <!-- keep me -->
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn merging_replaces_only_our_properties() {
        let merged = merge(DARKTABLE, d(Flag::Pick, 5)).unwrap();

        assert_eq!(read_decision(&merged).unwrap(), Some(d(Flag::Pick, 5)));
        assert!(merged.contains(r#"xmp:Rating="5""#), "{merged}");
        assert!(!merged.contains(r#"xmp:Rating="2""#));
        for kept in [
            r#"darktable:xmp_version="5""#,
            r#"darktable:history_end="3""#,
            "<rdf:li>holiday</rdf:li>",
            "<!-- keep me -->",
            "XMP Core 4.4.0-Exiv2",
        ] {
            assert!(merged.contains(kept), "lost {kept}:\n{merged}");
        }
        // The existing xmp prefix is reused; new namespaces are declared once.
        assert_eq!(merged.matches("xmlns:xmp=").count(), 1);
        assert_eq!(merged.matches("xmlns:digiKam=").count(), 1);
    }

    #[test]
    fn merging_twice_gives_the_same_result() {
        let once = merge(DARKTABLE, d(Flag::Reject, 1)).unwrap();
        let twice = merge(&once, d(Flag::Reject, 1)).unwrap();

        assert_eq!(once, twice);
    }

    #[test]
    fn element_form_properties_and_other_prefixes_are_replaced() {
        let lightroom = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xap="http://ns.adobe.com/xap/1.0/" xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/">
   <xap:Rating>3</xap:Rating>
   <crs:Exposure2012>+0.35</crs:Exposure2012>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

        assert_eq!(
            read_decision(lightroom).unwrap(),
            Some(d(Flag::Unflagged, 3))
        );
        let merged = merge(lightroom, d(Flag::Unflagged, 1)).unwrap();

        assert!(!merged.contains("<xap:Rating>3</xap:Rating>"), "{merged}");
        assert!(merged.contains(r#"xap:Rating="1""#), "{merged}");
        assert!(merged.contains("<crs:Exposure2012>+0.35</crs:Exposure2012>"));
        assert_eq!(read_decision(&merged).unwrap(), Some(d(Flag::Unflagged, 1)));
    }

    #[test]
    fn reads_decisions_written_by_other_tools() {
        let rejected = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="-1"/></rdf:RDF></x:xmpmeta>"#;
        let digikam_pick = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:digiKam="http://www.digikam.org/ns/1.0/" xmlns:xmp="http://ns.adobe.com/xap/1.0/" digiKam:PickLabel="3" xmp:Rating="4"/></rdf:RDF></x:xmpmeta>"#;
        let nothing = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description/></rdf:RDF></x:xmpmeta>"#;

        assert_eq!(read_decision(rejected).unwrap(), Some(d(Flag::Reject, 0)));
        assert_eq!(read_decision(digikam_pick).unwrap(), Some(d(Flag::Pick, 4)));
        assert_eq!(read_decision(nothing).unwrap(), None);
    }

    #[test]
    fn broken_or_unexpected_files_are_errors() {
        assert!(merge("<x:xmpmeta", d(Flag::Pick, 1)).is_err());
        assert!(merge("<root/>", d(Flag::Pick, 1)).is_err());
        assert!(read_decision("<a><b></a>").is_err());
    }

    #[test]
    fn a_taken_prefix_gets_a_fresh_one() {
        let odd = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:picturesque="urn:something-else" picturesque:Flag="mine"/></rdf:RDF></x:xmpmeta>"#;

        let merged = merge(odd, d(Flag::Pick, 2)).unwrap();

        assert!(merged.contains(r#"picturesque:Flag="mine""#), "{merged}");
        assert!(merged.contains("xmlns:picturesque1="), "{merged}");
        assert_eq!(read_decision(&merged).unwrap(), Some(d(Flag::Pick, 2)));
    }

    fn v(flag: Flag, stars: u8, label: Option<ColorLabel>) -> SidecarValues {
        SidecarValues {
            decision: d(flag, stars),
            label,
        }
    }

    #[test]
    fn labels_are_written_for_darktable_lightroom_and_digikam() {
        let xml = new_sidecar_with(v(Flag::Pick, 0, Some(ColorLabel::Green)));

        assert!(xml.contains(r#"xmp:Label="Green""#), "{xml}");
        assert!(xml.contains(r#"digiKam:ColorLabel="4""#), "{xml}");
        assert!(xml.contains(r#"picturesque:Label="green""#), "{xml}");
        assert!(
            xml.contains("<darktable:colorlabels><rdf:Seq><rdf:li>2</rdf:li></rdf:Seq></darktable:colorlabels>"),
            "{xml}"
        );
        assert_eq!(read_labels(&xml).unwrap().darktable, [2]);
        assert_eq!(read_decision(&xml).unwrap(), Some(d(Flag::Pick, 0)));
    }

    /// darktable's own colour label (red) in element form.
    const DARKTABLE_RED: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:darktable="http://darktable.sf.net/" darktable:xmp_version="5">
   <darktable:colorlabels>
    <rdf:Seq>
     <rdf:li>0</rdf:li>
    </rdf:Seq>
   </darktable:colorlabels>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn our_label_joins_and_leaves_other_labels_alone() {
        let picked =
            merge_values(DARKTABLE_RED, v(Flag::Pick, 0, Some(ColorLabel::Green))).unwrap();
        assert_eq!(read_labels(&picked).unwrap().darktable, [0, 2], "{picked}");
        assert!(picked.contains(r#"darktable:xmp_version="5""#));
        assert_eq!(picked.matches("xmlns:darktable=").count(), 1, "{picked}");

        let unpicked = merge_values(&picked, v(Flag::Unflagged, 0, None)).unwrap();
        let labels = read_labels(&unpicked).unwrap();
        assert_eq!(labels.darktable, [0], "{unpicked}");
        assert_eq!(labels.ours, None);
        assert!(!unpicked.contains("xmp:Label"), "{unpicked}");
        assert!(!unpicked.contains("digiKam:ColorLabel"), "{unpicked}");
    }

    #[test]
    fn a_label_set_in_another_tool_is_kept_when_ours_goes() {
        let lightroom = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Label="Red"/></rdf:RDF></x:xmpmeta>"#;

        let unchanged = merge_values(lightroom, v(Flag::Unflagged, 0, None)).unwrap();

        assert!(unchanged.contains(r#"xmp:Label="Red""#), "{unchanged}");
    }

    #[test]
    fn changing_the_label_replaces_ours_only() {
        let yellow = merge_values(
            DARKTABLE_RED,
            v(Flag::Unflagged, 0, Some(ColorLabel::Yellow)),
        )
        .unwrap();
        let green = merge_values(&yellow, v(Flag::Pick, 0, Some(ColorLabel::Green))).unwrap();

        assert_eq!(read_labels(&green).unwrap().darktable, [0, 2], "{green}");
        assert!(green.contains(r#"xmp:Label="Green""#));
    }

    #[test]
    fn merging_labels_twice_gives_the_same_result() {
        let once = merge_values(DARKTABLE_RED, v(Flag::Pick, 2, Some(ColorLabel::Purple))).unwrap();
        let twice = merge_values(&once, v(Flag::Pick, 2, Some(ColorLabel::Purple))).unwrap();

        assert_eq!(once, twice);
    }

    #[test]
    fn the_store_creates_merges_and_reads_sidecars() {
        use picturesque_app::ports::sidecars::SidecarStore;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("IMG_1.CR3.xmp");

        let (hash, size) = XmpSidecars.write(&path, v(Flag::Pick, 3, None)).unwrap();
        assert_eq!(XmpSidecars.read(&path).unwrap(), Some(d(Flag::Pick, 3)));
        assert_eq!(size, std::fs::metadata(&path).unwrap().len());
        assert_eq!(
            hash.as_bytes(),
            blake3::hash(&std::fs::read(&path).unwrap()).as_bytes()
        );

        std::fs::write(&path, DARKTABLE).unwrap();
        XmpSidecars.write(&path, v(Flag::Reject, 0, None)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("<rdf:li>holiday</rdf:li>"));
        assert_eq!(XmpSidecars.read(&path).unwrap(), Some(d(Flag::Reject, 0)));
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no temp files left"
        );
    }

    #[test]
    fn an_unparseable_sidecar_is_left_alone() {
        use picturesque_app::ports::sidecars::SidecarStore;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.xmp");
        std::fs::write(&path, "<not-xmp").unwrap();

        assert!(XmpSidecars.write(&path, v(Flag::Pick, 1, None)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<not-xmp");
    }
}
