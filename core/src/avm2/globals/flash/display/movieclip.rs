//! `flash.display.MovieClip` builtin/prototype

use crate::avm2::activation::Activation;
use crate::avm2::array::ArrayStorage;
use crate::avm2::class::Class;
use crate::avm2::globals::flash::display::{framelabel, scene};
use crate::avm2::method::Method;
use crate::avm2::names::{Namespace, QName};
use crate::avm2::object::{ArrayObject, Object, TObject};
use crate::avm2::string::AvmString;
use crate::avm2::traits::Trait;
use crate::avm2::value::Value;
use crate::avm2::Error;
use crate::display_object::{MovieClip, Scene, TDisplayObject};
use crate::tag_utils::{SwfMovie, SwfSlice};
use gc_arena::{GcCell, MutationContext};
use std::sync::Arc;

/// Implements `flash.display.MovieClip`'s instance constructor.
pub fn instance_init<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(this) = this {
        activation.super_init(this, &[])?;

        if this.as_display_object().is_none() {
            let movie = Arc::new(SwfMovie::empty(activation.context.swf.version()));
            let new_do = MovieClip::new(SwfSlice::empty(movie), activation.context.gc_context);

            this.init_display_object(activation.context.gc_context, new_do.into());
        }
    }
    Ok(Value::Undefined)
}

/// Implements `flash.display.MovieClip`'s class constructor.
pub fn class_init<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    _this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    Ok(Value::Undefined)
}

/// Implements `addFrameScript`, an undocumented method of `MovieClip` used to
/// specify what methods of a clip's class run on which frames.
pub fn add_frame_script<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        for (frame_id, callable) in args.chunks_exact(2).map(|s| (s[0].clone(), s[1].clone())) {
            let frame_id = frame_id.coerce_to_u32(activation)? as u16 + 1;
            let callable = callable.coerce_to_object(activation)?;

            mc.register_frame_script(frame_id, callable, &mut activation.context);
        }
    } else {
        log::error!("Attempted to add frame scripts to non-MovieClip this!");
    }

    Ok(Value::Undefined)
}

/// Implements `currentFrame`.
pub fn current_frame<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        if let Some(Scene {
            name: _,
            start,
            length: _,
        }) = mc.current_scene()
        {
            return Ok(((mc.current_frame() + 1) - start).into());
        } else {
            return Ok(mc.current_frame().into());
        }
    }

    Ok(Value::Undefined)
}

/// Implements `currentFrameLabel`.
pub fn current_frame_label<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        return Ok(mc
            .current_label()
            .and_then(|(label, start_frame)| {
                if start_frame < mc.current_frame() {
                    None
                } else {
                    Some(AvmString::new(activation.context.gc_context, label).into())
                }
            })
            .unwrap_or(Value::Null));
    }

    Ok(Value::Undefined)
}

/// Implements `currentLabel`.
pub fn current_label<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        return Ok(mc
            .current_label()
            .map(|(label, _start_frame)| {
                AvmString::new(activation.context.gc_context, label).into()
            })
            .unwrap_or(Value::Null));
    }

    Ok(Value::Undefined)
}

/// Given a scene, produce its name, length, and a list of frame labels.
///
/// The intended purpose of this output is to be sent directly into the
/// constructor of `flash.display.Scene`.
fn labels_for_scene<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    mc: MovieClip<'gc>,
    scene: &Scene,
) -> Result<(String, u16, Object<'gc>), Error> {
    let Scene {
        name: scene_name,
        start: scene_start,
        length: scene_length,
    } = scene;
    let mut frame_labels = Vec::new();
    let frame_label_proto = activation.context.avm2.prototypes().framelabel;

    for (name, frame) in mc.labels_in_range(*scene_start, scene_start + scene_length) {
        let name: Value<'gc> = AvmString::new(activation.context.gc_context, name).into();
        let local_frame = frame - scene_start + 1;
        let args = [name, local_frame.into()];
        let frame_label = frame_label_proto.construct(activation, &args)?;

        framelabel::instance_init(activation, Some(frame_label), &args)?;

        frame_labels.push(Some(frame_label.into()));
    }

    Ok((
        scene_name.to_string(),
        *scene_length,
        ArrayObject::from_array(
            ArrayStorage::from_storage(frame_labels),
            activation.context.avm2.prototypes().array,
            activation.context.gc_context,
        ),
    ))
}

/// Implements `currentLabels`.
pub fn current_labels<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        let scene = mc.current_scene().unwrap_or_else(|| Scene {
            name: "".to_string(),
            start: 0,
            length: mc.total_frames(),
        });
        return Ok(labels_for_scene(activation, mc, &scene)?.2.into());
    }

    Ok(Value::Undefined)
}

/// Implements `currentScene`.
pub fn current_scene<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        let scene = mc.current_scene().unwrap_or_else(|| Scene {
            name: "".to_string(),
            start: 0,
            length: mc.total_frames(),
        });
        let (scene_name, scene_length, scene_labels) = labels_for_scene(activation, mc, &scene)?;
        let scene_proto = activation.context.avm2.prototypes().scene;
        let args = [
            AvmString::new(activation.context.gc_context, scene_name).into(),
            scene_labels.into(),
            scene_length.into(),
        ];

        let scene = scene_proto.construct(activation, &args)?;

        scene::instance_init(activation, Some(scene), &args)?;

        return Ok(scene.into());
    }

    Ok(Value::Undefined)
}

/// Implements `scenes`.
pub fn scenes<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        let mut scene_objects = Vec::new();
        let mut mc_scenes = mc.scenes();

        if mc.scenes().is_empty() {
            mc_scenes.push(Scene {
                name: "".to_string(),
                start: 0,
                length: mc.total_frames(),
            });
        }

        for scene in mc_scenes {
            let (scene_name, scene_length, scene_labels) =
                labels_for_scene(activation, mc, &scene)?;
            let scene_proto = activation.context.avm2.prototypes().scene;
            let args = [
                AvmString::new(activation.context.gc_context, scene_name).into(),
                scene_labels.into(),
                scene_length.into(),
            ];

            let scene = scene_proto.construct(activation, &args)?;

            scene::instance_init(activation, Some(scene), &args)?;

            scene_objects.push(Some(scene.into()));
        }

        return Ok(ArrayObject::from_array(
            ArrayStorage::from_storage(scene_objects),
            activation.context.avm2.prototypes().array,
            activation.context.gc_context,
        )
        .into());
    }

    Ok(Value::Undefined)
}

/// Implements `framesLoaded`.
pub fn frames_loaded<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        return Ok(mc.frames_loaded().into());
    }

    Ok(Value::Undefined)
}

/// Implements `isPlaying`.
pub fn is_playing<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        return Ok((mc.programmatically_played() && mc.playing()).into());
    }

    Ok(Value::Undefined)
}

/// Implements `totalFrames`.
pub fn total_frames<'gc>(
    _activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        return Ok(mc.total_frames().into());
    }

    Ok(Value::Undefined)
}

/// Implements `gotoAndPlay`.
pub fn goto_and_play<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        mc.set_programmatically_played(activation.context.gc_context);
        goto_frame(activation, mc, args, false)?;
    }

    Ok(Value::Undefined)
}

/// Implements `gotoAndStop`.
pub fn goto_and_stop<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        goto_frame(activation, mc, args, true)?;
    }

    Ok(Value::Undefined)
}

pub fn goto_frame<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    mc: MovieClip<'gc>,
    args: &[Value<'gc>],
    stop: bool,
) -> Result<(), Error> {
    let frame_or_label = args.get(0).cloned().unwrap_or(Value::Null);

    let scene = match args.get(1).cloned().unwrap_or(Value::Null) {
        Value::Null => None,
        v => mc
            .scene_label_to_number(&v.coerce_to_string(activation)?)
            .map(|v| v.saturating_sub(1)),
    }
    .unwrap_or(0) as u32;
    let frame = match frame_or_label {
        Value::Integer(i) => i as u32 + scene,
        Value::Unsigned(i) => i + scene,
        frame_or_label => {
            let frame_or_label = frame_or_label.coerce_to_string(activation)?;
            if let Ok(frame) = frame_or_label.parse::<u32>() {
                frame + scene
            } else {
                if let Some(scene) = args.get(1).cloned() {
                    //If the user specified a scene, we need to validate that
                    //the requested frame exists within that scene.
                    let scene = scene.coerce_to_string(activation)?;
                    if !mc.frame_exists_within_scene(&frame_or_label, &scene) {
                        return Err(format!(
                            "ArgumentError: Frame label {} not found in scene {}",
                            frame_or_label, scene
                        )
                        .into());
                    }
                }

                mc.frame_label_to_number(&frame_or_label).ok_or_else(|| {
                    format!(
                        "ArgumentError: {} is not a valid frame label.",
                        frame_or_label
                    )
                })? as u32
            }
        }
    };

    mc.goto_frame(&mut activation.context, frame as u16, stop);

    Ok(())
}

/// Implements `stop`.
pub fn stop<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        mc.stop(&mut activation.context);
    }

    Ok(Value::Undefined)
}

/// Implements `play`.
pub fn play<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        mc.set_programmatically_played(activation.context.gc_context);
        mc.play(&mut activation.context);
    }

    Ok(Value::Undefined)
}

/// Implements `prevFrame`.
pub fn prev_frame<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        mc.prev_frame(&mut activation.context);
    }

    Ok(Value::Undefined)
}

/// Implements `nextFrame`.
pub fn next_frame<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        mc.next_frame(&mut activation.context);
    }

    Ok(Value::Undefined)
}

/// Implements `prevScene`.
pub fn prev_scene<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        if let Some(Scene {
            name: _,
            start,
            length: _,
        }) = mc.previous_scene()
        {
            mc.goto_frame(&mut activation.context, start, false);
        }
    }

    Ok(Value::Undefined)
}

/// Implements `nextScene`.
pub fn next_scene<'gc>(
    activation: &mut Activation<'_, 'gc, '_>,
    this: Option<Object<'gc>>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error> {
    if let Some(mc) = this
        .and_then(|o| o.as_display_object())
        .and_then(|dobj| dobj.as_movie_clip())
    {
        if let Some(Scene {
            name: _,
            start,
            length: _,
        }) = mc.next_scene()
        {
            mc.goto_frame(&mut activation.context, start, false);
        }
    }

    Ok(Value::Undefined)
}

/// Construct `MovieClip`'s class.
pub fn create_class<'gc>(mc: MutationContext<'gc, '_>) -> GcCell<'gc, Class<'gc>> {
    let class = Class::new(
        QName::new(Namespace::package("flash.display"), "MovieClip"),
        Some(QName::new(Namespace::package("flash.display"), "Sprite").into()),
        Method::from_builtin(instance_init),
        Method::from_builtin(class_init),
        mc,
    );

    let mut write = class.write(mc);

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "addFrameScript"),
        Method::from_builtin(add_frame_script),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "currentFrame"),
        Method::from_builtin(current_frame),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "currentFrameLabel"),
        Method::from_builtin(current_frame_label),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "currentLabel"),
        Method::from_builtin(current_label),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "currentLabels"),
        Method::from_builtin(current_labels),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "currentScene"),
        Method::from_builtin(current_scene),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "scenes"),
        Method::from_builtin(scenes),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "framesLoaded"),
        Method::from_builtin(frames_loaded),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "isPlaying"),
        Method::from_builtin(is_playing),
    ));

    write.define_instance_trait(Trait::from_getter(
        QName::new(Namespace::public(), "totalFrames"),
        Method::from_builtin(total_frames),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "gotoAndPlay"),
        Method::from_builtin(goto_and_play),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "gotoAndStop"),
        Method::from_builtin(goto_and_stop),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "stop"),
        Method::from_builtin(stop),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "play"),
        Method::from_builtin(play),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "prevFrame"),
        Method::from_builtin(prev_frame),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "nextFrame"),
        Method::from_builtin(next_frame),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "prevScene"),
        Method::from_builtin(prev_scene),
    ));

    write.define_instance_trait(Trait::from_method(
        QName::new(Namespace::public(), "nextScene"),
        Method::from_builtin(next_scene),
    ));

    class
}
