//! Scans a calendar blob into a node arena.

pub(super) struct Node<'a> {
    pub(super) name: &'a str,
    pub(super) attrs: &'a str,
    pub(super) children: Vec<usize>,
}

pub(super) struct Arena<'a> {
    pub(super) nodes: Vec<Node<'a>>,
    pub(super) roots: Vec<usize>,
}

pub(super) enum ScanError {
    Syntax(String),
    TooDeep,
}

#[derive(Clone, Copy)]
enum NodeState {
    ChildrenOrClose,
    Children,
    Close,
}

struct Frame {
    node: usize,
    state: NodeState,
}

#[expect(clippy::too_many_lines, reason = "one scanner loop over the text")]
pub(super) fn scan(text: &str) -> Result<Arena<'_>, ScanError> {
    let bytes = text.as_bytes();
    let mut position = 0;
    let mut arena = Arena {
        nodes: Vec::new(),
        roots: Vec::new(),
    };
    let mut stack: Vec<Frame> = Vec::new();

    loop {
        skip_whitespace(bytes, &mut position);
        let Some(frame) = stack.last() else {
            if position == bytes.len() {
                return Ok(arena);
            }
            if bytes.get(position).copied() != Some(b'(') {
                return Err(ScanError::Syntax(
                    "text outside a top-level node".to_owned(),
                ));
            }
            let node = scan_node_header(text, &mut position)?;
            let index = arena.nodes.len();
            arena.nodes.push(node);
            arena.roots.push(index);
            stack.push(Frame {
                node: index,
                state: NodeState::ChildrenOrClose,
            });
            continue;
        };

        match frame.state {
            NodeState::ChildrenOrClose => match bytes.get(position).copied() {
                Some(b'(') => {
                    position += 1;
                    if let Some(frame) = stack.last_mut() {
                        frame.state = NodeState::Children;
                    }
                }
                Some(b')') => {
                    position += 1;
                    stack.pop();
                }
                Some(_) => {
                    return Err(ScanError::Syntax(
                        "expected a children group or node close".to_owned(),
                    ));
                }
                None => return Err(ScanError::Syntax("truncated node".to_owned())),
            },
            NodeState::Children => match bytes.get(position).copied() {
                Some(b')') => {
                    position += 1;
                    if let Some(frame) = stack.last_mut() {
                        frame.state = NodeState::Close;
                    }
                }
                Some(b'(') => {
                    if stack.len() >= 64 {
                        return Err(ScanError::TooDeep);
                    }
                    let parent = frame.node;
                    let node = scan_node_header(text, &mut position)?;
                    let index = arena.nodes.len();
                    arena.nodes.push(node);
                    let Some(parent_node) = arena.nodes.get_mut(parent) else {
                        return Err(ScanError::Syntax("invalid node parent".to_owned()));
                    };
                    parent_node.children.push(index);
                    stack.push(Frame {
                        node: index,
                        state: NodeState::ChildrenOrClose,
                    });
                }
                Some(_) => {
                    return Err(ScanError::Syntax(
                        "expected a child node or children-group close".to_owned(),
                    ));
                }
                None => return Err(ScanError::Syntax("truncated children group".to_owned())),
            },
            NodeState::Close => match bytes.get(position).copied() {
                Some(b')') => {
                    position += 1;
                    stack.pop();
                }
                Some(_) => return Err(ScanError::Syntax("expected a node close".to_owned())),
                None => return Err(ScanError::Syntax("truncated node".to_owned())),
            },
        }
    }
}

fn scan_node_header<'a>(text: &'a str, position: &mut usize) -> Result<Node<'a>, ScanError> {
    let bytes = text.as_bytes();
    if bytes.get(*position).copied() != Some(b'(') {
        return Err(ScanError::Syntax("expected a node".to_owned()));
    }
    *position += 1;
    skip_whitespace(bytes, position);

    let number_start = *position;
    while bytes.get(*position).is_some_and(u8::is_ascii_digit) {
        *position += 1;
    }
    if *position == number_start {
        return Err(ScanError::Syntax("node number is missing".to_owned()));
    }
    skip_whitespace(bytes, position);
    let separator_end = position.saturating_add(2);
    if bytes.get(*position..separator_end) != Some(b"||") {
        return Err(ScanError::Syntax("node is missing ||".to_owned()));
    }
    *position = separator_end;

    let name_start = *position;
    while let Some(byte) = bytes.get(*position).copied() {
        if byte == b'(' {
            break;
        }
        *position += 1;
    }
    if bytes.get(*position).copied() != Some(b'(') {
        return Err(ScanError::Syntax("node attributes are missing".to_owned()));
    }
    let Some(raw_name) = text.get(name_start..*position) else {
        return Err(ScanError::Syntax("node name is not valid UTF-8".to_owned()));
    };
    let name = trim_ascii(raw_name);
    if name.is_empty() {
        return Err(ScanError::Syntax("node name is missing".to_owned()));
    }
    *position += 1;

    let attrs_start = *position;
    while let Some(byte) = bytes.get(*position).copied() {
        if byte == b')' {
            break;
        }
        *position += 1;
    }
    if bytes.get(*position).copied() != Some(b')') {
        return Err(ScanError::Syntax(
            "node attributes are truncated".to_owned(),
        ));
    }
    let Some(attrs) = text.get(attrs_start..*position) else {
        return Err(ScanError::Syntax(
            "node attributes are not valid UTF-8".to_owned(),
        ));
    };
    *position += 1;

    Ok(Node {
        name,
        attrs,
        children: Vec::new(),
    })
}

/// P6 writes a line break inside a value as two DEL bytes (0x7F 0x7F; seen in OBS and SCHEDOPTIONS values of public samples, never inside a `clndr_data`), so DEL counts
/// as whitespace alongside ASCII whitespace.
const fn is_blob_whitespace(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte == 0x7F
}

fn skip_whitespace(bytes: &[u8], position: &mut usize) {
    while bytes
        .get(*position)
        .is_some_and(|byte| is_blob_whitespace(*byte))
    {
        *position += 1;
    }
}

fn trim_ascii(text: &str) -> &str {
    text.trim_matches(|character: char| character.is_ascii() && is_blob_whitespace(character as u8))
}
