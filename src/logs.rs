use std::collections::HashSet;
use std::fmt::Write;
use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Local, SecondsFormat};
use serde::Deserialize;
use tracing::Level;

#[derive(Debug, Clone)]
pub struct Logs {
    pub inner: Arc<Mutex<LogsInner>>,
}

pub type SpanId = u64;
pub type MessageId = u64;

#[derive(Debug, Clone)]
pub struct LogsInner {
    pub root_span: SpanId,
    pub spans: BTreeMap<SpanId, SpanEntry>,
    pub messages: BTreeMap<MessageId, MessageEntry>,

    pub last_query: Option<Query>,
    pub cur_string: Option<Arc<String>>,

    pub next_span_id: SpanId,
    pub next_message_id: MessageId,

    // An interner and some interned strings
    pub interner: Interner,
    /// "message"
    pub i_message: IString,
    /// "name"
    pub i_name: IString,
    /// ""
    pub i_empty: IString,
}

#[derive(Debug, Clone)]
pub struct SpanEntry {
    pub name: IString,
    pub fields: PseudoMap<IString, IValue>,
    pub events: Vec<EventEntry>,
    pub json_subspan_keys: HashMap<PseudoMap<IString, IValue>, SpanId>,
}

#[derive(Debug, Clone)]
pub enum EventEntry {
    Span(SpanId),
    Message(MessageId),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub timestamp: Option<DateTime<Local>>,
    pub level: Option<Level>,
    pub fields: PseudoMap<IString, IValue>,
    pub _target: IString,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Query {
    All,
    Span(SpanId),
}

pub fn print_indent(output: &mut String, depth: usize) {
    write!(output, "{:indent$}", "", indent = depth * 4).unwrap();
}
pub fn print_val(output: &mut String, _depth: usize, val: &IValue) {
    match val {
        IValue::S(v) => write!(output, "{}", v).unwrap(),
        IValue::B(v) => write!(output, "{}", v).unwrap(),
        IValue::I(v) => write!(output, "{}", v).unwrap(),
        IValue::F(v) => write!(output, "{}", v).unwrap(),
    }
}

pub fn print_span_header(output: &mut String, depth: usize, span: &SpanEntry) {
    if !span.name.is_empty() {
        print_indent(output, depth);
        write!(output, "[{}", span.name).unwrap();
        for (k, v) in &span.fields.vals {
            write!(output, ", {k} = ").unwrap();
            print_val(output, depth, v);
        }
        writeln!(output, "]").unwrap();
    }
}

pub fn print_span_recursive(
    this: &LogsInner,
    output: &mut String,
    depth: usize,
    span: &SpanEntry,
    range: Option<Range<usize>>,
) {
    print_span_header(output, depth, span);

    let event_range = if let Some(range) = range {
        &span.events[range]
    } else {
        &span.events[..]
    };
    for event in event_range {
        match event {
            EventEntry::Message(message_id) => {
                let entry = &this.messages[message_id];
                let message = entry
                    .fields
                    .vals
                    .iter()
                    .find(|(k, _v)| k == &this.i_message);
                print_indent(output, depth + 1);
                if let Some(level) = entry.level {
                    write!(output, "[{:5}] ", level).unwrap();
                } else {
                    write!(output, "      ").unwrap();
                }
                if let Some(timestamp) = &entry.timestamp {
                    write!(
                        output,
                        "[{}] ",
                        timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
                    )
                    .unwrap();
                }
                for (k, v) in &entry.fields.vals {
                    if k != &this.i_message {
                        write!(output, "[{} = ", k).unwrap();
                        print_val(output, depth, v);
                        write!(output, "] ").unwrap();
                    }
                }
                if let Some(message) = message {
                    print_val(output, depth + 1, &message.1);
                }
                writeln!(output).unwrap();
            }
            EventEntry::Span(sub_span) => {
                print_span_recursive(this, output, depth + 1, &this.spans[sub_span], None);
            }
        }
    }
}

impl Logs {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogsInner::new())),
        }
    }

    pub fn clear(&self) {
        let mut log = self.inner.lock().unwrap();
        let root_span = log.root_span;
        let mut root = log.spans.remove(&root_span).unwrap();
        root.events.clear();

        log.spans.clear();
        log.messages.clear();
        log.cur_string = None;
        log.next_message_id = 0;
        log.next_span_id = 1;

        log.spans.insert(root_span, root);
    }

    pub fn add_json_message(&self, input: &str) {
        self.inner.lock().unwrap().add_json_message(input);
    }

    pub fn string_query(&self, query: Query) -> Arc<String> {
        let mut log = self.inner.lock().unwrap();
        if Some(query) == log.last_query {
            if let Some(string) = &log.cur_string {
                return string.clone();
            }
        }
        log.last_query = Some(query);

        let mut output = String::new();

        let (span_to_print, range) = match query {
            Query::All => (&log.spans[&log.root_span], None),
            Query::Span(span) => (&log.spans[&span], None),
        };

        print_span_recursive(&log, &mut output, 0, span_to_print, range);

        let result = Arc::new(output);
        log.cur_string = Some(result.clone());
        result
    }
}

impl Default for Logs {
    fn default() -> Self {
        Self::new()
    }
}

impl LogsInner {
    pub fn new() -> Self {
        const ROOT_SPAN: SpanId = 0;
        const JSON_MESSAGE_KEY: &str = "message";
        const JSON_SPAN_NAME_KEY: &str = "name";
        const ROOT_SPAN_NAME: &str = "<all spans>";

        let empty = IString(Arc::from(""));

        let mut this = Self {
            root_span: ROOT_SPAN,
            spans: BTreeMap::new(),
            messages: BTreeMap::new(),
            last_query: None,
            cur_string: None,
            next_span_id: 1,
            next_message_id: 0,
            i_message: empty.clone(),
            i_name: empty.clone(),
            i_empty: empty,
            interner: Interner::default(),
        };

        let root_span = SpanEntry {
            name: this.interner.intern_str(ROOT_SPAN_NAME),
            fields: PseudoMap::default(),
            events: Vec::new(),
            json_subspan_keys: HashMap::new(),
        };
        this.spans.insert(ROOT_SPAN, root_span);

        this.i_message = this.interner.intern_str(JSON_MESSAGE_KEY);
        this.i_name = this.interner.intern_str(JSON_SPAN_NAME_KEY);
        this.i_empty = this.interner.intern_str("");

        this
    }

    pub fn add_json_message(&mut self, input: &str) {
        let json_message = match serde_json::from_str::<JsonMessage>(input) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("WARN: failed to parse log line: {}\n{}", input, e);
                return;
            }
        };
        let mut cur_span_id = self.root_span;
        for json_span in json_message.spans {
            let cur_span = self.spans.get_mut(&cur_span_id).unwrap();
            let i_json_span = self.interner.intern_pseudo(json_span);
            cur_span_id = match cur_span.json_subspan_keys.entry(i_json_span) {
                std::collections::hash_map::Entry::Occupied(e) => *e.get(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    // Make a new span
                    let new_span_id = self.next_span_id;
                    self.next_span_id += 1;

                    // This is done in a weird way because if you have a span with "name" key
                    // then tracing will emit two keys with the string "name". We assume the
                    // one we want is first.
                    let name = if let Some((k, IValue::S(name))) = e.key().vals.last() {
                        if k == &self.i_name {
                            name.clone()
                        } else {
                            self.i_empty.clone()
                        }
                    } else {
                        self.i_empty.clone()
                    };
                    let mut fields = e.key().clone();
                    if !name.is_empty() {
                        fields.vals.pop();
                    }
                    let new_span = SpanEntry {
                        name,
                        fields,
                        events: Vec::new(),
                        json_subspan_keys: HashMap::new(),
                    };

                    e.insert(new_span_id);
                    cur_span.events.push(EventEntry::Span(new_span_id));
                    self.spans.insert(new_span_id, new_span);

                    new_span_id
                }
            };
        }

        let span = self.spans.get_mut(&cur_span_id).unwrap();
        let new_message_id = self.next_message_id;
        self.next_message_id += 1;
        let new_message = MessageEntry {
            timestamp: json_message.timestamp.parse().ok(),
            level: match json_message.level {
                "ERROR" => Some(Level::ERROR),
                "WARN" => Some(Level::WARN),
                "INFO" => Some(Level::INFO),
                "DEBUG" => Some(Level::DEBUG),
                "TRACE" => Some(Level::TRACE),
                _ => None,
            },
            _target: self.interner.intern_str(json_message.target),
            fields: self.interner.intern_pseudo(json_message.fields),
        };
        self.messages.insert(new_message_id, new_message);
        span.events.push(EventEntry::Message(new_message_id));
    }
}

impl Default for LogsInner {
    fn default() -> Self {
        Self::new()
    }
}

// A string interner that makes all strings share the same Arc,
// so they can be compared by address and deduplicated.
#[derive(Debug, Clone, Default)]
pub struct Interner {
    facts: HashMap<IString, StringInfo>,
    strings: HashSet<Arc<str>>,
}

impl Interner {
    pub fn intern_str(&mut self, val: &str) -> IString {
        if let Some(k) = self.strings.get(val) {
            IString(k.clone())
        } else {
            let num_lines = val.lines().count();
            let k = Arc::from(val);
            self.strings.insert(Arc::clone(&k));
            self.facts
                .insert(IString(Arc::clone(&k)), StringInfo { num_lines });
            IString(k)
        }
    }
    pub fn intern_val(&mut self, val: Value) -> IValue {
        match val {
            Value::S(v) => IValue::S(self.intern_str(&v)),
            Value::B(v) => IValue::B(v),
            Value::I(v) => IValue::I(v),
            Value::F(v) => IValue::F(v),
        }
    }
    pub fn intern_pseudo(&mut self, val: PseudoMap<&str, Value>) -> PseudoMap<IString, IValue> {
        PseudoMap {
            vals: val
                .vals
                .into_iter()
                .map(|(k, v)| (self.intern_str(k), self.intern_val(v)))
                .collect(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
struct JsonMessage<'a> {
    timestamp: &'a str,
    level: &'a str,
    fields: PseudoMap<&'a str, Value>,
    target: &'a str,
    #[serde(default)]
    spans: Vec<JsonSpan<'a>>,
}

type JsonSpan<'a> = PseudoMap<&'a str, Value>;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum Value {
    S(String),
    B(bool),
    I(i64),
    F(EqF64),
}

/// An interned string, where hashing/equality or by-address
#[derive(Clone)]
pub struct IString(Arc<str>);
impl std::ops::Deref for IString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PartialEq for IString {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}
impl Eq for IString {}
impl std::hash::Hash for IString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}
impl std::fmt::Debug for IString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::fmt::Display for IString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Default, Debug, Clone)]
pub struct StringInfo {
    pub num_lines: usize,
}

#[derive(Copy, Clone, Deserialize)]
pub struct EqF64(f64);

impl std::fmt::Debug for EqF64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::fmt::Display for EqF64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::hash::Hash for EqF64 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_le_bytes().hash(state);
    }
}
impl PartialEq for EqF64 {
    fn eq(&self, other: &EqF64) -> bool {
        self.0.to_le_bytes() == other.0.to_le_bytes()
    }
}
impl Eq for EqF64 {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IValue {
    S(IString),
    B(bool),
    I(i64),
    F(EqF64),
}

/// This is kind of a map but `tracing` can end up with `name` twice so it's just `Vec<(K, V)>`
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct PseudoMap<K, V> {
    pub vals: Vec<(K, V)>,
}

impl<K, V> Default for PseudoMap<K, V> {
    fn default() -> Self {
        Self {
            vals: Default::default(),
        }
    }
}

#[test]
fn test_parse_json_message_no_spans() {
    let input = r###"{"timestamp":"2022-02-15T18:47:10.821315Z","level":"INFO","fields":{"message":"preparing to shave yaks","number_of_yaks":3},"target":"fmt_json"}"###;

    let _json_message: JsonMessage = serde_json::from_str(input).unwrap();
}

#[test]
fn test_parse_json_message_spans() {
    let input = r###"{"timestamp":"2022-02-15T18:47:10.821495Z","level":"TRACE","fields":{"message":"hello! I'm gonna shave a yak","excitement":"yay!"},"target":"fmt_json::yak_shave","spans":[{"yaks":3,"name":"shaving_yaks"},{"yak":1,"name":"shave"}]}"###;

    let _json_message: JsonMessage = serde_json::from_str(input).unwrap();
}

#[test]
fn test_parse_json_message_dupe_name() {
    let input = r###"{"timestamp":"2022-02-15T18:47:10.821495Z","level":"TRACE","fields":{"message":"hello! I'm gonna shave a yak","excitement":"yay!"},"target":"fmt_json::yak_shave","spans":[{"name": "real_name", "yaks":3,"name":"shaving_yaks"}]}"###;

    let json_message: JsonMessage = serde_json::from_str(input).unwrap();
    assert_eq!(
        &json_message.spans[0].vals[0],
        &("name", Value::S("real_name".to_owned()))
    );
    assert_eq!(&json_message.spans[0].vals[1], &("yak", Value::I(3)));
    assert_eq!(
        &json_message.spans[0].vals[2],
        &("name", Value::S("shaving_yaks".to_owned()))
    );
}

use std::fmt;
use std::marker::PhantomData;

impl<K, V> PseudoMap<K, V> {
    fn with_capacity(cap: usize) -> Self {
        Self {
            vals: Vec::with_capacity(cap),
        }
    }
}

// A Visitor is a type that holds methods that a Deserializer can drive
// depending on what is contained in the input data.
//
// In the case of a map we need generic type parameters K and V to be
// able to set the output type correctly, but don't require any state.
// This is an example of a "zero sized type" in Rust. The PhantomData
// keeps the compiler from complaining about unused generic type
// parameters.
struct MyMapVisitor<K, V> {
    marker: PhantomData<fn() -> PseudoMap<K, V>>,
}

impl<K, V> MyMapVisitor<K, V> {
    fn new() -> Self {
        MyMapVisitor {
            marker: PhantomData,
        }
    }
}

// This is the trait that Deserializers are going to be driving. There
// is one method for each type of data that our type knows how to
// deserialize from. There are many other methods that are not
// implemented here, for example deserializing from integers or strings.
// By default those methods will return an error, which makes sense
// because we cannot deserialize a MyMap from an integer or string.
impl<'de, K, V> serde::de::Visitor<'de> for MyMapVisitor<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    // The type that our Visitor is going to produce.
    type Value = PseudoMap<K, V>;

    // Format a message stating what data this Visitor expects to receive.
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a very special map")
    }

    // Deserialize MyMap from an abstract "map" provided by the
    // Deserializer. The MapAccess input is a callback provided by
    // the Deserializer to let us see each entry in the map.
    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: serde::de::MapAccess<'de>,
    {
        let mut map = PseudoMap::with_capacity(access.size_hint().unwrap_or(0));

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_entry()? {
            map.vals.push((key, value));
        }

        Ok(map)
    }
}

// This is the trait that informs Serde how to deserialize MyMap.
impl<'de, K, V> Deserialize<'de> for PseudoMap<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        // Instantiate our Visitor and ask the Deserializer to drive
        // it over the input data, resulting in an instance of MyMap.
        deserializer.deserialize_map(MyMapVisitor::new())
    }
}
