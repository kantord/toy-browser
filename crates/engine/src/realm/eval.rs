//! Running JavaScript on demand, and the handle table that keeps its results
//! alive.
//!
//! Everything a caller asks the Realm to run after the load arrives here.
//! A result either comes back as a JSON copy or stays in the runtime behind a
//! [`Handle`], which is what makes it possible to pass an object back in later.

use rquickjs::{Ctx, Function, Persistent, Value};

use crate::Mode;

use super::{
    Realm,
    convert::{exception_text, js_to_json, json_to_js},
};

impl Realm {
    /// Evaluates `expression` and describes the result.
    pub fn evaluate(&self, expression: &str, mode: Mode) -> Evaluated {
        self.context.with(|ctx| {
            match ctx.eval::<Value, _>(expression) {
                Ok(value) => {
                    // Promises are settled before describing the result, so a
                    // caller that asked for a value never receives a pending one.
                    let value = self.settle(value);
                    self.describe(&ctx, value, mode)
                }
                Err(error) => Evaluated::Threw(exception_text(&ctx, error)),
            }
        })
    }

    /// Calls `declaration` — the source of a function expression — with `this`
    /// bound to `receiver` and the given arguments.
    pub fn call(
        &self,
        declaration: &str,
        receiver: Option<&Handle>,
        arguments: &[Argument],
        mode: Mode,
    ) -> Evaluated {
        self.context.with(
            |ctx| match self.invoke(&ctx, declaration, receiver, arguments) {
                Ok(value) => {
                    let value = self.settle(value);
                    self.describe(&ctx, value, mode)
                }
                Err(message) => Evaluated::Threw(message),
            },
        )
    }

    /// Compiles the declaration, assembles the call and makes it.
    ///
    /// Every way this can fail ends as the same thing — a message for
    /// [`Evaluated::Threw`] — so they travel as one `Err` rather than as five
    /// early returns the caller has to read past. What comes back is the raw
    /// result; settling and describing it is [`Realm::call`]'s job.
    fn invoke<'js>(
        &self,
        ctx: &Ctx<'js>,
        declaration: &str,
        receiver: Option<&Handle>,
        arguments: &[Argument],
    ) -> Result<Value<'js>, String> {
        let function: Function = ctx
            .eval(format!("({declaration})"))
            .map_err(|error| exception_text(ctx, error))?;

        let mut call_args = rquickjs::function::Args::new(ctx.clone(), arguments.len());
        let this = receiver
            .and_then(|handle| self.restore(ctx, handle))
            .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
        call_args
            .this(this)
            .map_err(|_| "could not bind `this`".to_owned())?;

        for argument in arguments {
            call_args
                .push_arg(self.argument(ctx, argument)?)
                .map_err(|_| "could not pass argument".to_owned())?;
        }

        function
            .call_arg::<Value>(call_args)
            .map_err(|error| exception_text(ctx, error))
    }

    /// One argument as a JavaScript value: a JSON copy, or whatever a handle
    /// still stands for — undefined, if it stands for nothing.
    fn argument<'js>(&self, ctx: &Ctx<'js>, argument: &Argument) -> Result<Value<'js>, String> {
        match argument {
            Argument::Value(json) => {
                json_to_js(ctx, json).map_err(|error| exception_text(ctx, error))
            }
            Argument::Handle(handle) => Ok(self
                .restore(ctx, handle)
                .unwrap_or_else(|| Value::new_undefined(ctx.clone()))),
        }
    }

    /// Forgets a retained value.
    pub fn release(&self, handle: &Handle) {
        self.handles.borrow_mut().remove(&handle.0);
    }

    /// Runs the job queue until a promise resolves, leaving other values alone.
    fn settle<'js>(&self, value: Value<'js>) -> Value<'js> {
        let Some(promise) = value.clone().into_promise() else {
            return value;
        };
        promise.finish::<Value>().unwrap_or(value)
    }

    /// Turns a JavaScript value into either a JSON copy or a retained handle.
    fn describe<'js>(&self, ctx: &Ctx<'js>, value: Value<'js>, mode: Mode) -> Evaluated {
        // Primitives have no identity worth keeping, so they are copied even
        // when a caller asked for a reference.
        if mode == Mode::ByValue || !(value.is_object() || value.is_function()) {
            return match js_to_json(ctx, &value) {
                Ok(json) => Evaluated::Value(json),
                Err(error) => Evaluated::Threw(exception_text(ctx, error)),
            };
        }

        let id = format!("h{}", self.next_handle.replace(self.next_handle.get() + 1));
        self.handles
            .borrow_mut()
            .insert(id.clone(), Persistent::save(ctx, value));
        Evaluated::Handle(Handle(id))
    }

    fn restore<'js>(&self, ctx: &Ctx<'js>, handle: &Handle) -> Option<Value<'js>> {
        self.handles
            .borrow()
            .get(&handle.0)
            .map(|saved| saved.clone().restore(ctx))
            .transpose()
            .ok()
            .flatten()
    }
}

/// A retained reference to a JavaScript value.
///
/// Lives until released or until its Realm is replaced. The string inside is
/// opaque; nothing but the Realm that issued it can make sense of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle(String);

impl Handle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Handle {
    fn from(id: String) -> Self {
        Self(id)
    }
}

/// An argument to a call: either a literal or something already retained.
#[derive(Debug, Clone)]
pub enum Argument {
    Value(serde_json::Value),
    Handle(Handle),
}

/// The result of running JavaScript.
#[derive(Debug, Clone)]
pub enum Evaluated {
    /// A JSON copy of the result.
    Value(serde_json::Value),
    /// A retained result, because it has identity worth keeping.
    Handle(Handle),
    /// The message of whatever was thrown, with its stack when there was one.
    Threw(String),
}
