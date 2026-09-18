//! What has to be true of a rasterizer that anybody may talk to.
//!
//! Two questions, and they are different. **Who can reach it** — because what
//! crosses this socket is every word of every page somebody has open. And
//! **what one client can learn about another** — because the bytes are held
//! once for everybody, and the saving is only safe if the permission to name
//! them is not shared along with them.

mod common;

use common::{listening, ours, red, settled};

/// What crosses this socket is every word of every page somebody has open. So
/// the two questions worth asking of it are who can connect, and who could have
/// put it there.
mod reachable_only_by_its_owner {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    use toy_browser_rasterizer::wire::{Client, default_socket, serve};

    use super::{ours, settled};

    fn mode(at: &std::path::Path) -> u32 {
        std::fs::metadata(at)
            .expect("it exists")
            .permissions()
            .mode()
            & 0o777
    }

    /// `bind` takes the umask, which is usually 0022 — so a socket comes out
    /// connectable by every user on the machine unless something narrows it.
    #[test]
    fn the_socket_and_its_directory_are_the_owner_s_alone() {
        let directory = ours().join("private");
        let _ = std::fs::remove_dir_all(&directory);
        let socket = directory.join("raster.sock");
        let there = socket.clone();
        std::thread::spawn(move || serve(&there));
        settled(&socket);

        assert_eq!(mode(&directory), 0o700, "nobody else may enter");
        assert_eq!(mode(&socket), 0o600, "nobody else may connect");
    }

    /// The attack this is really about: another user creates the directory
    /// first, with a mode that lets them put a socket in it, and is handed
    /// every Scene the browser draws. Both ends refuse, rather than one of them
    /// succeeding quietly.
    #[test]
    fn a_directory_anybody_could_have_written_is_refused() {
        let directory = ours().join("open");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(&directory)
            .expect("a world-readable directory");
        let socket = directory.join("raster.sock");

        assert!(serve(&socket).is_err(), "a server will not listen there");
        assert!(
            Client::connect(&socket).is_err(),
            "and a client will not speak there, which is the half that matters: \
             it is the one that would have sent the page"
        );
    }

    /// Never the temporary directory itself, which is world-writable.
    #[test]
    fn the_default_is_never_somewhere_anybody_can_write() {
        let socket = default_socket();
        let directory = socket.parent().expect("it has one");
        assert_ne!(
            directory,
            std::env::temp_dir(),
            "a socket directly in the temporary directory is one anybody can \
             create first and be handed the screen"
        );
    }
}

/// What a rasterizer shared by clients that do not trust each other must and
/// must not do.
///
/// The bytes are held once for everybody — a typeface is megabytes and every
/// client sends the same handful — and the permission to name them is per
/// connection. These check that the second half is real, because the first half
/// is what makes it tempting not to be.
mod between_clients_that_do_not_trust_each_other {
    use toy_browser_rasterizer::wire::Client;
    use toy_browser_rasterizer::{Digest, Face, Mark, Scene};

    use super::{listening, red};

    /// A Scene that draws nothing but names a face, so the only question is
    /// whether the server will resolve it.
    fn naming(face: Digest) -> Scene {
        Scene {
            marks: vec![Mark::Glyphs {
                places: vec![],
                text: String::new(),
                glyphs: vec![],
                baseline: 0.0,
                size: 10.0,
                paint: red(),
                face,
                from: None,
            }],
            width: 4,
            height: 4,
            ..Scene::default()
        }
    }

    /// The oracle this is all about: a client must not be able to find out that
    /// something exists by naming it.
    ///
    /// One client sends a secret. Another names its digest — which it could
    /// only do by having guessed the content, but the whole point is that
    /// guessing must not pay. It is told the same thing it would be told about
    /// a digest nobody has ever sent.
    #[test]
    fn one_client_cannot_name_what_another_sent() {
        let socket = listening("apart");
        let secret: std::sync::Arc<[u8]> = b"a private document".to_vec().into();
        let digest = Digest::of(&secret);

        let mut owner = Client::connect(&socket).expect("connect");
        let mut held = naming(digest);
        held.faces.insert(digest, Face { bytes: secret });
        owner.draw(&held).expect("its owner may draw it");

        // A second connection, naming the same digest and holding nothing.
        let mut stranger = Client::connect(&socket).expect("connect");
        let refused = stranger.draw(&naming(digest));
        assert!(
            refused.is_err(),
            "a client that never sent it must not be able to name it"
        );

        // And the same answer for something nobody has ever sent, so the two
        // are indistinguishable.
        let never = Digest::of(b"nobody has ever drawn this");
        let unknown = stranger.draw(&naming(never));
        assert!(unknown.is_err(), "which is what an unknown digest gets too");
    }

    /// The line the whole model rests on. A client that sends the digest of one
    /// thing with the bytes of another would, unchecked, replace it for
    /// everybody: content addressing is only addressing by content if somebody
    /// checks.
    #[test]
    fn bytes_that_are_not_what_their_digest_says_are_refused() {
        let socket = listening("lying");
        let mut liar = Client::connect(&socket).expect("connect");
        let claimed = Digest::of(b"what everybody else calls this");
        let mut scene = naming(claimed);
        scene.faces.insert(
            claimed,
            Face {
                bytes: b"but these are my bytes".to_vec().into(),
            },
        );
        assert!(
            liar.draw(&scene).is_err(),
            "a digest that does not match its bytes is refused"
        );
    }

    /// A client asking for more pixels than anything could want is refused
    /// rather than allowed to decide how much memory this process uses.
    #[test]
    fn a_scene_too_large_to_draw_is_refused_rather_than_attempted() {
        let socket = listening("huge");
        let mut greedy = Client::connect(&socket).expect("connect");
        let scene = Scene {
            width: 200_000,
            height: 200_000,
            ..Scene::default()
        };
        assert!(greedy.draw(&scene).is_err());
    }
}

/// What one client gets of another's *work* — as opposed to another's bytes.
///
/// A rasterizer serving many clients fills each glyph once for all of them: the
/// atlas key is a face Digest and four numbers, so the same letter at the same
/// size in the same face is the same entry whoever asked. That sharing needs no
/// permission of its own, and that is the whole point — reaching an entry means
/// naming a face, and naming a face means having sent it. **Access to a derived
/// thing is access to what it was derived from**, which is already settled.
mod work_is_shared_because_its_inputs_were_proved {
    use toy_browser_rasterizer::filled_so_far;

    use super::common::{a_public_face, lettered};

    /// Drawing the same words twice fills them once.
    ///
    /// In one process here rather than over two connections, because what is
    /// being shown is that the atlas is process-wide rather than per caller —
    /// and a second connection is a second caller in the same process, which is
    /// exactly the case that used to have an atlas of its own.
    #[test]
    fn the_same_glyphs_are_filled_once_however_often_they_are_asked_for() {
        let bytes = std::fs::read(a_public_face()).expect("a font").into();
        let scene = lettered("shared", bytes);
        let before = filled_so_far();
        toy_browser_rasterizer::pixels(&scene).expect("first");
        let once = filled_so_far();
        toy_browser_rasterizer::pixels(&scene).expect("second");
        let twice = filled_so_far();

        assert!(once > before, "the first drawing fills them");
        assert_eq!(twice, once, "and the second fills nothing at all");
    }
}

/// A client must not be able to decide how long this process spends, or how
/// much memory it uses. On a rasterizer shared between clients, the one that
/// stops is everybody.
mod nothing_one_client_sends_can_take_the_others_down {
    use toy_browser_rasterizer::{Area, Digest, Format, Mark, Picture, Scene};

    /// A Scene that draws one picture in a ten-pixel box.
    ///
    /// Ten pixels because that is the point: where a Mark puts a picture says
    /// nothing about what the file claims to be, so the Scene's own size limit
    /// does not cover this at all.
    fn showing(picture: Picture) -> Scene {
        let digest = Digest::of(&picture.bytes);
        let mut scene = Scene {
            width: 10,
            height: 10,
            ..Scene::default()
        };
        scene.pictures.insert(digest, picture);
        scene.marks.push(Mark::Image {
            area: Area {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            picture: digest,
            from: None,
        });
        scene
    }

    fn took(scene: &Scene) -> std::time::Duration {
        let began = std::time::Instant::now();
        toy_browser_rasterizer::pixels(scene).expect("it draws, with or without the picture");
        began.elapsed()
    }

    /// A hundred and twenty bytes that took thirty-five seconds before the
    /// guard, in a ten-pixel box.
    ///
    /// This is the one that was real. The raster version below is not, and both
    /// are here so that the difference stays measured rather than remembered.
    #[test]
    fn an_svg_that_declares_itself_enormous_is_never_drawn() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"><rect width="100%" height="100%" fill="red"/></svg>"#;
        let scene = showing(Picture {
            bytes: svg.to_vec().into(),
            format: Format::Svg,
        });
        assert!(
            took(&scene) < std::time::Duration::from_secs(5),
            "refused on its declared size rather than drawn"
        );
    }

    /// And the raster equivalent, which needs nothing from us: the decoder's own
    /// limits refuse it, and they are shaped better than a limit here would be —
    /// a cap on what is allocated rather than on an edge, so a legitimate
    /// panorama still draws.
    ///
    /// Kept because it is the evidence for *not* having written that limit.
    #[test]
    fn a_raster_that_declares_itself_enormous_is_refused_by_its_decoder() {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&u16::MAX.to_le_bytes());
        bytes.extend_from_slice(&u16::MAX.to_le_bytes());
        bytes.extend_from_slice(&[0xF7, 0x00, 0x00]);
        bytes.extend(std::iter::repeat_n(0u8, 3 * 256));
        bytes.extend_from_slice(b"\x2C");
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.extend_from_slice(&u16::MAX.to_le_bytes());
        bytes.extend_from_slice(&u16::MAX.to_le_bytes());
        bytes.extend_from_slice(&[0x00, 0x08]);
        let scene = showing(Picture {
            bytes: bytes.into(),
            format: Format::Gif,
        });
        assert!(took(&scene) < std::time::Duration::from_secs(5));
    }
}
