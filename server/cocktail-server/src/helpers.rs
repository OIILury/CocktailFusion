use handlebars::{
  html_escape, Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext,
  RenderError, ScopedJson,
};
use ory_kratos_client::models::{UiNode, UiNodeAttributes};
use serde_json::json;

pub struct OnlyNodes;

impl HelperDef for OnlyNodes {
  fn call_inner<'reg: 'rc, 'rc>(
    &self,
    h: &Helper<'reg, 'rc>,
    _: &'reg Handlebars<'reg>,
    _: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
  ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
    let nodes = h
      .param(0)
      .map(|p| p.value())
      .ok_or_else(|| RenderError::new("param nodes not found"))?;
    // let only = h.param(1).map(|p| p.value());
    // dbg!(&nodes);
    let nodes: Vec<UiNode> = serde_json::from_value(nodes.to_owned())?;
    // let only = only.and_then(|o| serde_json::from_value::<String>(o.clone()).ok());
    let n = json!(nodes);
    println!("{n}");
    Ok(ScopedJson::Derived(json!(nodes)))
  }

  fn call<'reg: 'rc, 'rc>(
    &self,
    h: &Helper<'reg, 'rc>,
    r: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    rc: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn handlebars::Output,
  ) -> handlebars::HelperResult {
    match self.call_inner(h, r, ctx, rc) {
      Ok(result) => {
        if r.strict_mode() && result.is_missing() {
          Err(RenderError::strict_error(None))
        } else {
          // auto escape according to settings
          let output = html_escape(&result.render());
          out.write(output.as_ref())?;
          Ok(())
        }
      }
      Err(_) => {
        Ok(())
        // if e.is_unimplemented() {
        //     // default implementation, do nothing
        //     Ok(())
        // } else {
        //     Err(e)
        // }
      }
    }
  }
}
pub fn to_ui_node_partial(
  h: &Helper,
  _: &Handlebars,
  _: &Context,
  _: &mut RenderContext,
  out: &mut dyn Output,
) -> HelperResult {
  let node = h
    .param(0)
    .map(|p| p.value())
    .ok_or_else(|| RenderError::new("param node not found"))?;

  let node = serde_json::from_value::<UiNode>(node.to_owned())?;
  // dbg!(&node);
  match *node.attributes {
    UiNodeAttributes::UiNodeAnchorAttributes { .. } => out.write("ui_node_anchor.html")?,
    UiNodeAttributes::UiNodeImageAttributes { .. } => out.write("ui_node_image.html")?,
    UiNodeAttributes::UiNodeInputAttributes { _type, .. } => match _type.as_ref() {
      "hidden" => out.write("ui_node_input_hidden.html")?,
      "submit" => out.write("ui_node_input_button.html")?,
      "button" => out.write("ui_node_input_button.html")?,
      "checkbox" => out.write("ui_node_input_checkbox.html")?,
      _ => out.write("ui_node_input_default.html")?,
    },
    UiNodeAttributes::UiNodeScriptAttributes { .. } => out.write("ui_node_script.html")?,
    UiNodeAttributes::UiNodeTextAttributes { .. } => out.write("ui_node_text.html")?,
  };

  Ok(())
}

pub fn get_node_label(
  h: &Helper,
  _: &Handlebars,
  _: &Context,
  _: &mut RenderContext,
  out: &mut dyn Output,
) -> HelperResult {
  let node = h
    .param(0)
    .map(|p| p.value())
    .ok_or_else(|| RenderError::new("param node not found"))?;

  let node = serde_json::from_value::<UiNode>(node.to_owned())?;

  // let label = node.meta.label.map(|l| l.text).unwrap_or_default();
  // out.write(&label)?;

  match *node.attributes {
    UiNodeAttributes::UiNodeAnchorAttributes { title, .. } => out.write(&*title.text)?,
    UiNodeAttributes::UiNodeImageAttributes { .. } => {
      let label = node.meta.label.map(|l| l.text).unwrap_or_default();
      out.write(&label)?
    }
    UiNodeAttributes::UiNodeInputAttributes { label, .. } => {
      let label = label.map(|l| match l.id {
        1070001 => "mot de passe".to_string(),
        _ => l.text,
      });
      if let Some(text) = label {
        out.write(&text)?
      } else {
        let label = node
          .meta
          .label
          .map(|l| match l.id {
            // TODO c'est pas génial mais ça fait le taf
            1070001 => "mot de passe".to_string(),
            _ => l.text,
          })
          .unwrap_or_default();
        out.write(&label)?
      }
    }
    _ => {
      let label = node.meta.label.map(|l| l.text).unwrap_or_default();
      out.write(&label)?
    }
  };

  Ok(())
}
