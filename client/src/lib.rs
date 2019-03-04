pub mod error;
pub mod input;

// Reexports
pub use specs::Entity as EcsEntity;
pub use crate::{
    error::Error,
    input::Input,
};

use std::{
    time::Duration,
    net::SocketAddr,
};
use vek::*;
use threadpool;
use specs::Builder;
use common::{
    comp,
    state::State,
    terrain::TerrainChunk,
    net::PostBox,
    msg::{ClientMsg, ServerMsg},
};
use world::World;

pub enum Event {
    Chat(String),
}

pub struct Client {
    thread_pool: threadpool::ThreadPool,

    last_ping: f64,
    postbox: PostBox<ClientMsg, ServerMsg>,

    tick: u64,
    state: State,
    player: Option<EcsEntity>,

    // Testing
    world: World,
    pub chunk: Option<TerrainChunk>,
}

impl Client {
    /// Create a new `Client`.
    #[allow(dead_code)]
    pub fn new<A: Into<SocketAddr>>(addr: A) -> Result<Self, Error> {
        let state = State::new();

        let mut postbox = PostBox::to_server(addr)?;
        postbox.send(ClientMsg::Chat(String::from("Hello, world!")));

        Ok(Self {
            thread_pool: threadpool::Builder::new()
                .thread_name("veloren-worker".into())
                .build(),

            last_ping: state.get_time(),
            postbox,

            tick: 0,
            state,
            player: None,

            // Testing
            world: World::new(),
            chunk: None,
        })
    }

    /// Get a reference to the client's worker thread pool. This pool should be used for any
    /// computationally expensive operations that run outside of the main thread (i.e: threads that
    /// block on I/O operations are exempt).
    #[allow(dead_code)]
    pub fn thread_pool(&self) -> &threadpool::ThreadPool { &self.thread_pool }

    // TODO: Get rid of this
    pub fn with_test_state(mut self) -> Self {
        self.chunk = Some(self.world.generate_chunk(Vec3::zero()));
        self.player = Some(self.state.new_test_player());
        self
    }

    // TODO: Get rid of this
    pub fn load_chunk(&mut self, pos: Vec3<i32>) {
        self.state.terrain_mut().insert(pos, self.world.generate_chunk(pos));
        self.state.changes_mut().new_chunks.push(pos);
    }

    /// Get a reference to the client's game state.
    #[allow(dead_code)]
    pub fn state(&self) -> &State { &self.state }

    /// Get a mutable reference to the client's game state.
    #[allow(dead_code)]
    pub fn state_mut(&mut self) -> &mut State { &mut self.state }

    /// Get an entity from its UID, creating it if it does not exists
    pub fn get_or_create_entity(&mut self, uid: u64) -> EcsEntity {
        self.state.ecs_world_mut().create_entity()
            .with(comp::Uid(uid))
            .build()
    }

    /// Get the player entity
    #[allow(dead_code)]
    pub fn player(&self) -> Option<EcsEntity> {
        self.player
    }

    /// Get the current tick number.
    #[allow(dead_code)]
    pub fn get_tick(&self) -> u64 {
        self.tick
    }

    /// Send a chat message to the server
    #[allow(dead_code)]
    pub fn send_chat(&mut self, msg: String) -> Result<(), Error> {
        Ok(self.postbox.send(ClientMsg::Chat(msg))?)
    }

    /// Execute a single client tick, handle input and update the game state by the given duration
    #[allow(dead_code)]
    pub fn tick(&mut self, input: Input, dt: Duration) -> Result<Vec<Event>, Error> {
        // This tick function is the centre of the Veloren universe. Most client-side things are
        // managed from here, and as such it's important that it stays organised. Please consult
        // the core developers before making significant changes to this code. Here is the
        // approximate order of things. Please update it as this code changes.
        //
        // 1) Collect input from the frontend, apply input effects to the state of the game
        // 2) Go through any events (timer-driven or otherwise) that need handling and apply them
        //    to the state of the game
        // 3) Perform a single LocalState tick (i.e: update the world and entities in the world)
        // 4) Go through the terrain update queue and apply all changes to the terrain
        // 5) Finish the tick, passing control of the main thread back to the frontend

        // Build up a list of events for this frame, to be passed to the frontend
        let mut frontend_events = Vec::new();

        // Handle new messages from the server
        frontend_events.append(&mut self.handle_new_messages()?);

        // Step 3
        if let Some(p) = self.player {
            // TODO: remove this
            const PLAYER_VELOCITY: f32 = 100.0;

            // TODO: Set acceleration instead
            self.state.write_component(p, comp::phys::Vel(Vec3::from(input.move_dir * PLAYER_VELOCITY)));
        }

        // Tick the client's LocalState (step 3)
        self.state.tick(dt);

        // Finish the tick, pass control back to the frontend (step 6)
        self.tick += 1;
        Ok(frontend_events)
    }

    /// Clean up the client after a tick
    #[allow(dead_code)]
    pub fn cleanup(&mut self) {
        // Cleanup the local state
        self.state.cleanup();
    }

    /// Handle new server messages
    fn handle_new_messages(&mut self) -> Result<Vec<Event>, Error> {
        let mut frontend_events = Vec::new();

        // Step 1
        let new_msgs = self.postbox.new_messages();

        if new_msgs.len() > 0 {
            self.last_ping = self.state.get_time();

            for msg in new_msgs {
                println!("Received message");
                match msg {
                    ServerMsg::Shutdown => return Err(Error::ServerShutdown),
                    ServerMsg::Chat(msg) => frontend_events.push(Event::Chat(msg)),
                    ServerMsg::EntityPhysics { uid, pos, vel, dir } => {
                        let ecs_entity = self.get_or_create_entity(uid);
                        self.state.write_component(ecs_entity, pos);
                        self.state.write_component(ecs_entity, vel);
                        self.state.write_component(ecs_entity, dir);
                    },
                }
            }
        } else if let Some(err) = self.postbox.status() {
            return Err(err.into());
        }

        Ok(frontend_events)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.postbox.send(ClientMsg::Disconnect).unwrap();
    }
}
