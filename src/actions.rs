//! Bounded local operations shared by every button. No domain-specific controls or code execution.
use crate::{Result, resolve};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(tag = "action", content = "params", deny_unknown_fields)]
enum Action {
    #[serde(rename = "setState")]
    Set {
        #[serde(rename = "statePath")]
        path: String,
        value: Value,
    },
    #[serde(rename = "compute")]
    Compute {
        #[serde(rename = "statePath")]
        path: String,
        op: String,
        args: Vec<Value>,
    },
}

fn parse(binding: &Value, state: &Value) -> Result<Vec<Action>> {
    let bindings = match binding {
        Value::Array(items) => items.clone(),
        item => vec![item.clone()],
    };
    if bindings.is_empty() || bindings.len() > 8 {
        return Err("A press requires 1–8 local operations".into());
    }
    let actions: Vec<Action> = bindings
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        .collect::<Result<_>>()?;
    for action in &actions {
        let path = match action {
            Action::Set { path, .. } | Action::Compute { path, .. } => path,
        };
        let target = state
            .pointer(path)
            .filter(|_| path.starts_with('/'))
            .ok_or("Action must target existing state")?;
        match action {
            Action::Set { value, .. } => {
                resolve(value, state)?;
            }
            Action::Compute { op, args, .. } => {
                let arity = match op.as_str() {
                    "add" | "subtract" | "multiply" | "divide" | "append" => 2,
                    "backspace" | "evaluate" => 1,
                    "randomInt" => 3,
                    _ => return Err("Unsupported compute operation".into()),
                };
                if args.len() != arity || !(target.is_string() || target.is_number()) {
                    return Err("Invalid operation arguments or target type".into());
                }
                for arg in args {
                    let value = resolve(arg, state)?;
                    if !(value.is_string() || value.is_number()) {
                        return Err(
                            "Operation arguments must be strings, numbers or state references"
                                .into(),
                        );
                    }
                }
            }
        }
    }
    Ok(actions)
}

pub fn validate(binding: &Value, state: &Value) -> Result<()> {
    parse(binding, state).map(|_| ())
}

fn number(value: &Value) -> Result<f64> {
    let n = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
        .ok_or("Enter a valid number")?;
    if !n.is_finite() || n.abs() > 1e9 {
        return Err("Number is out of bounds".into());
    }
    Ok(n)
}

fn text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

pub fn apply(binding: &Value, state: &Value) -> Result<Value> {
    apply_with_random(binding, state, &mut || {
        let mut bytes = [0; 8];
        getrandom::fill(&mut bytes).map_err(|_| "Random source unavailable".to_owned())?;
        Ok(u64::from_le_bytes(bytes))
    })
}

fn apply_with_random(
    binding: &Value,
    state: &Value,
    random: &mut impl FnMut() -> Result<u64>,
) -> Result<Value> {
    let actions = parse(binding, state)?;
    let mut next = state.clone();
    for action in actions {
        let (path, value) = match action {
            Action::Set { path, value } => (path, resolve(&value, &next)?),
            Action::Compute { path, op, args } => {
                let args = args
                    .iter()
                    .map(|v| resolve(v, &next))
                    .collect::<Result<Vec<_>>>()?;
                let value = match op.as_str() {
                    "append" => json!(format!("{}{}", text(&args[0]), text(&args[1]))),
                    "backspace" => {
                        let mut s = text(&args[0]);
                        s.pop();
                        json!(s)
                    }
                    "evaluate" => json!(
                        crate::calculator::eval(&text(&args[0]))
                            .ok_or("Invalid arithmetic expression")?
                    ),
                    "randomInt" => {
                        let (min, max, count) =
                            (number(&args[0])?, number(&args[1])?, number(&args[2])?);
                        if [min, max, count].iter().any(|v| v.fract() != 0.0)
                            || min < -1e6
                            || max > 1e6
                            || min > max
                            || !(1.0..=32.0).contains(&count)
                        {
                            return Err("randomInt requires integer min <= max within ±1000000 and count 1–32".into());
                        }
                        let range = (max - min + 1.0) as u64;
                        let limit = u64::MAX - u64::MAX % range;
                        let mut total = 0i64;
                        for _ in 0..count as usize {
                            let mut sample = random()?;
                            while sample >= limit {
                                sample = random()?;
                            }
                            total += min as i64 + (sample % range) as i64;
                        }
                        json!(total)
                    }
                    _ => {
                        let (a, b) = (number(&args[0])?, number(&args[1])?);
                        let n = match op.as_str() {
                            "add" => a + b,
                            "subtract" => a - b,
                            "multiply" => a * b,
                            "divide" if b != 0.0 => a / b,
                            _ => return Err("Cannot divide by zero".into()),
                        };
                        if !n.is_finite() || n.abs() > 1e9 {
                            return Err("Result is out of bounds".into());
                        }
                        json!(n)
                    }
                };
                let value = if next.pointer(&path).is_some_and(Value::is_string) {
                    json!(if let Some(n) = value.as_f64() {
                        n.to_string()
                    } else {
                        text(&value)
                    })
                } else {
                    json!(number(&value)?)
                };
                (path, value)
            }
        };
        if value.as_str().is_some_and(|s| s.len() > 4096) {
            return Err("Result exceeds 4096 bytes".into());
        }
        *next.pointer_mut(&path).ok_or("Missing action state")? = value;
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operations_compose_without_domain_specific_code() {
        let state = json!({"count":"2","sides":"6","modifier":"3","result":"Ready"});
        let actions = json!([
            {"action":"compute","params":{"statePath":"/result","op":"randomInt","args":[1,{"$state":"/sides"},{"$state":"/count"}]}},
            {"action":"compute","params":{"statePath":"/result","op":"add","args":[{"$state":"/result"},{"$state":"/modifier"}]}}
        ]);
        let mut samples = [0, 5].into_iter();
        assert_eq!(
            apply_with_random(&actions, &state, &mut || Ok(samples.next().unwrap())).unwrap()["result"],
            "10"
        );
        assert_eq!(state["result"], "Ready");
        let bad = json!([actions[0], {"action":"compute","params":{"statePath":"/result","op":"divide","args":[1,0]}}]);
        assert!(apply(&bad, &state).is_err());
        let expression = json!({"action":"compute","params":{"statePath":"/result","op":"evaluate","args":["7+8*2"]}});
        assert_eq!(apply(&expression, &state).unwrap()["result"], "23");
        assert!(
            validate(
                &json!({"action":"compute","params":{"statePath":"/result","op":"exec","args":[]}}),
                &state
            )
            .is_err()
        );
        let invalid = json!({"action":"compute","params":{"statePath":"/result","op":"randomInt","args":[1,6,1000]}});
        assert!(apply(&invalid, &state).is_err());
    }
}
