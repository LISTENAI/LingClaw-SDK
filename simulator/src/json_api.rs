use mlua::{Lua, LuaSerdeExt, Value};
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashSet};
fn encode(
    lua: &Lua,
    value: Value,
    seen: &mut HashSet<usize>,
    depth: usize,
) -> Result<Json, String> {
    if depth > 32 {
        return Err("too_deep".into());
    }
    Ok(match value {
        Value::LightUserData(p) if p.0.is_null() => Json::Null,
        Value::Boolean(b) => b.into(),
        Value::Integer(i) => i.into(),
        Value::Number(n) => serde_json::Number::from_f64(n)
            .map(Json::Number)
            .ok_or("invalid_value")?,
        Value::String(s) => {
            let s = s.to_str().map_err(|_| "invalid_value")?;
            if s.contains('\0') {
                return Err("invalid_value".into());
            }
            Json::String(s.to_string())
        }
        Value::Table(t) => {
            let identity = t.to_pointer() as usize;
            if !seen.insert(identity) {
                return Err("invalid_value".into());
            }
            let mut object = serde_json::Map::new();
            let mut array = BTreeMap::new();
            for pair in t.clone().pairs::<Value, Value>() {
                let (k, v) = pair.map_err(|_| "invalid_value")?;
                let v = encode(lua, v, seen, depth + 1)?;
                match k {
                    Value::String(s) => {
                        let s = s.to_str().map_err(|_| "invalid_value")?;
                        if s.contains('\0') {
                            return Err("invalid_value".into());
                        }
                        object.insert(s.to_string(), v);
                    }
                    Value::Integer(i) if i > 0 => {
                        array.insert(i, v);
                    }
                    _ => return Err("invalid_value".into()),
                }
            }
            seen.remove(&identity);
            if !array.is_empty() {
                if !object.is_empty() || *array.last_key_value().unwrap().0 != array.len() as i64 {
                    return Err("invalid_value".into());
                }
                Json::Array(array.into_values().collect())
            } else if object.is_empty()
                && t.metatable()
                    .is_some_and(|m| m.to_pointer() == lua.array_metatable().to_pointer())
            {
                Json::Array(vec![])
            } else {
                Json::Object(object)
            }
        }
        _ => return Err("invalid_value".into()),
    })
}
fn validate(value: &Json, depth: usize) -> Result<(), String> {
    if depth > 32 {
        return Err("too_deep".into());
    }
    match value {
        Json::String(s) if s.contains('\0') => return Err("invalid_json".into()),
        Json::Array(a) => {
            for v in a {
                validate(v, depth + 1)?;
            }
        }
        Json::Object(o) => {
            for (k, v) in o {
                if k.contains('\0') {
                    return Err("invalid_json".into());
                }
                validate(v, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}
pub fn register(lua: &Lua, max_bytes: usize) -> mlua::Result<()> {
    let table = lua.create_table()?;
    table.set("null", lua.null())?;
    table.set(
        "encode",
        lua.create_function(move |lua, value: Value| {
            let result = encode(lua, value, &mut HashSet::new(), 0)
                .and_then(|j| serde_json::to_string(&j).map_err(|_| "invalid_value".to_owned()))
                .and_then(|s| {
                    if s.len() > max_bytes {
                        Err("too_large".into())
                    } else {
                        Ok(s)
                    }
                });
            Ok(match result {
                Ok(s) => (Some(s), None),
                Err(e) => (None, Some(e)),
            })
        })?,
    )?;
    table.set(
        "decode",
        lua.create_function(move |lua, value: Value| {
            let result = (|| -> Result<Json, String> {
                let Value::String(s) = value else {
                    return Err("invalid_json".into());
                };
                if s.as_bytes().len() > max_bytes {
                    return Err("too_large".into());
                }
                let json: Json =
                    serde_json::from_slice(&s.as_bytes()).map_err(|_| "invalid_json")?;
                validate(&json, 0)?;
                Ok(json)
            })();
            match result {
                Ok(v) => Ok((lua.to_value(&v)?, None)),
                Err(e) => Ok((Value::Nil, Some(e))),
            }
        })?,
    )?;
    lua.globals().set("json", table)?;
    Ok(())
}
