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
