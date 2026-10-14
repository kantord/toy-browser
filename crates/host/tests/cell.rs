//! The `cell` op: how big a cell is, in CSS px, so a page is as wide as the
//! columns the client has. Absent or unusable means 8 by 16.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Host {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Host {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_toy-browser-host"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the host starts");
        let input = child.stdin.take().expect("stdin");
        let output = BufReader::new(child.stdout.take().expect("stdout"));
        let mut host = Self {
            child,
            input,
            output,
        };
        assert_eq!(host.next()["ev"], "hello");
        host
    }

    fn next(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).expect("a line");
        serde_json::from_str(&line).expect("json")
    }

    /// Sends a command and answers the text of the frame it leads to.
    fn ask(&mut self, command: &Value) -> String {
        writeln!(self.input, "{command}").expect("sent");
        loop {
            let event = self.next();
            if event["ev"] == "frame" {
                return event["lines"].to_string();
            }
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

const PAGE: &str = "<style>p::after{content:'narrow'} \
    @media (min-width: 1000px){p::after{content:'wide'}}</style><p></p>";

#[test]
fn the_cell_op_sets_the_width_the_page_sees() {
    let mut host = Host::start();
    host.ask(&json!({"op": "markup", "html": PAGE}));
    let narrow = host.ask(&json!({"op": "resize", "cols": 100, "rows": 5}));
    assert!(narrow.contains("narrow"), "{narrow}");

    let wide = host.ask(&json!({"op": "cell", "width": 10.0, "height": 20.0}));
    assert!(wide.contains("wide"), "{wide}");

    let unusable = host.ask(&json!({"op": "cell", "width": 0, "height": "tall"}));
    assert!(!unusable.contains("wide"), "{unusable}");
}
