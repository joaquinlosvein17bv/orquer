use std::time::Duration;

use crate::api::schema::mailbox::{
    MailboxAskParams, MailboxListParams, MailboxRecvParams, MailboxReplyParams, MailboxSendParams,
};
use crate::api::schema::{Method, Request};

pub(super) fn run_msg_command(args: &[String]) -> std::io::Result<i32> {
    let Some(subcommand) = args.first().map(|arg| arg.as_str()) else {
        print_msg_help();
        return Ok(2);
    };

    match subcommand {
        "send" => msg_send(&args[1..]),
        "recv" => msg_recv(&args[1..]),
        "ask" => msg_ask(&args[1..]),
        "reply" => msg_reply(&args[1..]),
        "list" => msg_list(&args[1..]),
        "help" | "--help" | "-h" => {
            print_msg_help();
            Ok(0)
        }
        _ => {
            print_msg_help();
            Ok(2)
        }
    }
}

pub(super) fn run_msg_ask_shorthand(args: &[String]) -> std::io::Result<i32> {
    msg_ask(&args[1..])
}

pub(super) fn run_msg_reply_shorthand(args: &[String]) -> std::io::Result<i32> {
    msg_reply(&args[1..])
}

fn current_identity(fallback: &str) -> String {
    std::env::var("ORQUER_PANE_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn parse_payload(raw: &str) -> serde_json::Value {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        v
    } else {
        serde_json::Value::String(raw.to_string())
    }
}

fn msg_send(args: &[String]) -> std::io::Result<i32> {
    let mut to = None;
    let mut from = None;
    let mut msg_type = "progress".to_string();
    let mut payload = serde_json::json!({});
    let mut correlation_id = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--to" => {
                if let Some(v) = args.get(i + 1) {
                    to = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --to");
                    return Ok(2);
                }
            }
            "--from" => {
                if let Some(v) = args.get(i + 1) {
                    from = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --from");
                    return Ok(2);
                }
            }
            "--type" => {
                if let Some(v) = args.get(i + 1) {
                    msg_type = v.clone();
                    i += 2;
                } else {
                    eprintln!("missing value for --type");
                    return Ok(2);
                }
            }
            "--payload" => {
                if let Some(v) = args.get(i + 1) {
                    payload = parse_payload(v);
                    i += 2;
                } else {
                    eprintln!("missing value for --payload");
                    return Ok(2);
                }
            }
            "--correlation-id" => {
                if let Some(v) = args.get(i + 1) {
                    correlation_id = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --correlation-id");
                    return Ok(2);
                }
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            "help" | "--help" | "-h" => {
                eprintln!("usage: orquer msg send --to <target> [--from <origin>] [--type <type>] [--payload <data>] [--correlation-id <id>] [--json]");
                return Ok(0);
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }

    let Some(to) = to else {
        eprintln!("error: --to is required");
        eprintln!("usage: orquer msg send --to <target> [--from <origin>] [--type <type>] [--payload <data>] [--correlation-id <id>] [--json]");
        return Ok(2);
    };

    let from = from.unwrap_or_else(|| current_identity("anonymous"));

    let request = Request {
        id: "cli:msg:send".to_string(),
        method: Method::MailboxSend(MailboxSendParams {
            to: to.clone(),
            from: Some(from),
            msg_type,
            payload,
            correlation_id,
        }),
    };

    let response = super::send_request(&request)?;
    if let Some(err) = response.get("error") {
        eprintln!("{}", serde_json::to_string(err).unwrap());
        return Ok(1);
    }

    if json_output {
        println!("{}", serde_json::to_string(&response).unwrap());
    } else if let Some(msg_id) = response
        .get("result")
        .and_then(|r| r.get("message_id"))
        .and_then(|id| id.as_str())
    {
        println!("{msg_id}");
    } else {
        println!("ok");
    }

    Ok(0)
}

fn msg_recv(args: &[String]) -> std::io::Result<i32> {
    let mut recipient = None;
    let mut from = None;
    let mut msg_type = None;
    let mut wait = false;
    let mut timeout_ms = None;
    let mut json_output = false;
    let mut raw_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--recipient" => {
                if let Some(v) = args.get(i + 1) {
                    recipient = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --recipient");
                    return Ok(2);
                }
            }
            "--from" => {
                if let Some(v) = args.get(i + 1) {
                    from = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --from");
                    return Ok(2);
                }
            }
            "--type" => {
                if let Some(v) = args.get(i + 1) {
                    msg_type = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --type");
                    return Ok(2);
                }
            }
            "--wait" => {
                wait = true;
                i += 1;
            }
            "--timeout-ms" => {
                if let Some(v) = args.get(i + 1) {
                    match v.parse::<u64>() {
                        Ok(num) => timeout_ms = Some(num),
                        Err(_) => {
                            eprintln!("invalid number for --timeout-ms: {v}");
                            return Ok(2);
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("missing value for --timeout-ms");
                    return Ok(2);
                }
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            "--raw" => {
                raw_output = true;
                i += 1;
            }
            "help" | "--help" | "-h" => {
                eprintln!("usage: orquer msg recv [--recipient <target>] [--from <origin>] [--type <type>] [--wait] [--timeout-ms <ms>] [--raw] [--json]");
                return Ok(0);
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }

    let recipient = recipient.unwrap_or_else(|| current_identity("orchestrator"));

    let request = Request {
        id: "cli:msg:recv".to_string(),
        method: Method::MailboxRecv(MailboxRecvParams {
            recipient: Some(recipient),
            from,
            msg_type,
            wait,
            timeout_ms,
        }),
    };

    let response = super::send_request(&request)?;
    if let Some(err) = response.get("error") {
        eprintln!("{}", serde_json::to_string(err).unwrap());
        return Ok(1);
    }

    let message = response.get("result").and_then(|r| r.get("message"));
    if message.is_none() || message == Some(&serde_json::Value::Null) {
        if json_output {
            println!("{}", serde_json::json!({ "message": null }));
        }
        return Ok(0);
    }

    let msg = message.unwrap();
    if json_output {
        println!("{}", serde_json::to_string(msg).unwrap());
    } else if raw_output {
        if let Some(p) = msg.get("payload") {
            if let Some(s) = p.as_str() {
                println!("{s}");
            } else {
                println!("{}", serde_json::to_string_pretty(p).unwrap());
            }
        }
    } else {
        let id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let from = msg.get("from").and_then(|v| v.as_str()).unwrap_or("");
        let m_type = msg.get("msg_type").and_then(|v| v.as_str()).unwrap_or("");
        println!("[{m_type}] from {from} (id: {id}):");
        if let Some(p) = msg.get("payload") {
            if let Some(s) = p.as_str() {
                println!("{s}");
            } else {
                println!("{}", serde_json::to_string_pretty(p).unwrap());
            }
        }
    }

    Ok(0)
}

fn msg_ask(args: &[String]) -> std::io::Result<i32> {
    let mut to = None;
    let mut from = None;
    let mut question = None;
    let mut options = Vec::new();
    let mut timeout_ms = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--to" => {
                if let Some(v) = args.get(i + 1) {
                    to = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --to");
                    return Ok(2);
                }
            }
            "--from" => {
                if let Some(v) = args.get(i + 1) {
                    from = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --from");
                    return Ok(2);
                }
            }
            "--question" => {
                if let Some(v) = args.get(i + 1) {
                    question = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --question");
                    return Ok(2);
                }
            }
            "--option" => {
                if let Some(v) = args.get(i + 1) {
                    options.push(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --option");
                    return Ok(2);
                }
            }
            "--options" => {
                if let Some(v) = args.get(i + 1) {
                    for opt in v.split(',') {
                        let trimmed = opt.trim();
                        if !trimmed.is_empty() {
                            options.push(trimmed.to_string());
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("missing value for --options");
                    return Ok(2);
                }
            }
            "--timeout-ms" => {
                if let Some(v) = args.get(i + 1) {
                    match v.parse::<u64>() {
                        Ok(num) => timeout_ms = Some(num),
                        Err(_) => {
                            eprintln!("invalid number for --timeout-ms: {v}");
                            return Ok(2);
                        }
                    }
                    i += 2;
                } else {
                    eprintln!("missing value for --timeout-ms");
                    return Ok(2);
                }
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            "help" | "--help" | "-h" => {
                eprintln!("usage: orquer msg ask --question <question> --option <opt1> [--option <opt2>...] [--to <target>] [--from <origin>] [--timeout-ms <ms>] [--json]");
                return Ok(0);
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }

    let Some(question) = question else {
        eprintln!("error: --question is required");
        eprintln!("usage: orquer msg ask --question <question> --option <opt1> [--option <opt2>...]");
        return Ok(2);
    };

    if options.is_empty() {
        eprintln!("error: at least one --option is required");
        eprintln!("usage: orquer msg ask --question <question> --option <opt1> [--option <opt2>...]");
        return Ok(2);
    }

    let to = to.unwrap_or_else(|| "orchestrator".to_string());
    let from = from.unwrap_or_else(|| current_identity("subordinate"));

    let request = Request {
        id: "cli:msg:ask".to_string(),
        method: Method::MailboxAsk(MailboxAskParams {
            to,
            from: Some(from),
            question,
            options,
            timeout_ms,
        }),
    };

    let response = super::send_request(&request)?;
    if let Some(err) = response.get("error") {
        eprintln!("{}", serde_json::to_string(err).unwrap());
        return Ok(1);
    }

    let reply = response.get("result").and_then(|r| r.get("message"));
    if let Some(reply) = reply {
        if json_output {
            println!("{}", serde_json::to_string(reply).unwrap());
        } else {
            let choice = reply
                .get("payload")
                .and_then(|p| p.get("choice"))
                .and_then(|c| c.as_str())
                .unwrap_or("");
            println!("{choice}");
        }
        Ok(0)
    } else {
        eprintln!("no reply received");
        Ok(1)
    }
}

fn msg_reply(args: &[String]) -> std::io::Result<i32> {
    let mut id = None;
    let mut choice = None;
    let mut author = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--id" => {
                if let Some(v) = args.get(i + 1) {
                    id = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --id");
                    return Ok(2);
                }
            }
            "--choice" => {
                if let Some(v) = args.get(i + 1) {
                    choice = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --choice");
                    return Ok(2);
                }
            }
            "--author" => {
                if let Some(v) = args.get(i + 1) {
                    author = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --author");
                    return Ok(2);
                }
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            "help" | "--help" | "-h" => {
                eprintln!("usage: orquer msg reply --id <question_id> --choice <selected_choice> [--author <name>] [--json]");
                return Ok(0);
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }

    let Some(question_id) = id else {
        eprintln!("error: --id is required");
        eprintln!("usage: orquer msg reply --id <question_id> --choice <selected_choice>");
        return Ok(2);
    };

    let Some(choice) = choice else {
        eprintln!("error: --choice is required");
        eprintln!("usage: orquer msg reply --id <question_id> --choice <selected_choice>");
        return Ok(2);
    };

    let author = author.unwrap_or_else(|| current_identity("orchestrator"));

    let request = Request {
        id: "cli:msg:reply".to_string(),
        method: Method::MailboxReply(MailboxReplyParams {
            question_id: question_id.clone(),
            choice: choice.clone(),
            author: Some(author),
        }),
    };

    let response = super::send_request(&request)?;
    if let Some(err) = response.get("error") {
        eprintln!("{}", serde_json::to_string(err).unwrap());
        return Ok(1);
    }

    if json_output {
        println!("{}", serde_json::to_string(&response).unwrap());
    } else {
        println!("replied to question {question_id} with choice: {choice}");
    }

    Ok(0)
}

fn msg_list(args: &[String]) -> std::io::Result<i32> {
    let mut recipient = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--recipient" => {
                if let Some(v) = args.get(i + 1) {
                    recipient = Some(v.clone());
                    i += 2;
                } else {
                    eprintln!("missing value for --recipient");
                    return Ok(2);
                }
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            "help" | "--help" | "-h" => {
                eprintln!("usage: orquer msg list [--recipient <target>] [--json]");
                return Ok(0);
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }

    let request = Request {
        id: "cli:msg:list".to_string(),
        method: Method::MailboxList(MailboxListParams { recipient }),
    };

    let response = super::send_request(&request)?;
    if let Some(err) = response.get("error") {
        eprintln!("{}", serde_json::to_string(err).unwrap());
        return Ok(1);
    }

    let messages = response.get("result").and_then(|r| r.get("messages"));
    if json_output {
        println!(
            "{}",
            serde_json::to_string(messages.unwrap_or(&serde_json::json!([]))).unwrap()
        );
    } else if let Some(msgs) = messages.and_then(|m| m.as_array()) {
        if msgs.is_empty() {
            println!("no queued messages");
        } else {
            println!(
                "{:<26} {:<16} {:<16} {:<10} {}",
                "ID", "FROM", "TO", "TYPE", "PAYLOAD"
            );
            for msg in msgs {
                let id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let from = msg.get("from").and_then(|v| v.as_str()).unwrap_or("");
                let to = msg.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let m_type = msg.get("msg_type").and_then(|v| v.as_str()).unwrap_or("");
                let payload = msg.get("payload").map(|p| p.to_string()).unwrap_or_default();
                let truncated = if payload.len() > 40 {
                    format!("{}...", &payload[..37])
                } else {
                    payload
                };
                println!(
                    "{:<26} {:<16} {:<16} {:<10} {}",
                    id, from, to, m_type, truncated
                );
            }
        }
    }

    Ok(0)
}

fn print_msg_help() {
    eprintln!("orquer msg commands:");
    eprintln!("  orquer msg send --to <target> [--from <origin>] [--type <type>] [--payload <data>] [--correlation-id <id>]");
    eprintln!("  orquer msg recv [--recipient <target>] [--from <origin>] [--type <type>] [--wait] [--timeout-ms <ms>] [--raw]");
    eprintln!("  orquer msg ask --question <question> --option <choice1> [--option <choice2>...] [--to <target>] [--timeout-ms <ms>]");
    eprintln!("  orquer msg reply --id <question_id> --choice <selected_choice> [--author <name>]");
    eprintln!("  orquer msg list [--recipient <target>]");
    eprintln!();
    eprintln!("Message types:");
    eprintln!("  task, progress, ask, reply, result, error");
}
