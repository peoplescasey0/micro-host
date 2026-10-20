//! Spatial Workspace for Post Card OS
//!
//! Implements the 3D Rolodex metaphor with dual-sided Post Cards
//! that can be pinned to physical walls and shared instantly.

use core::f32::consts::PI;
use spin::Mutex;

/// 3D spatial workspace manager
pub struct SpatialWorkspace {
    /// All post cards in the workspace
    cards: Mutex<Vec<PostCard>>,
    /// 3D Rolodex organization
    rolodex: Rolodex3D,
    /// Physics engine for interactions
    physics: PhysicsEngine,
    /// Spatial audio system
    audio: SpatialAudio,
}

/// Dual-sided Post Card
pub struct PostCard {
    /// Unique identifier
    id: u64,
    /// Front side: Application/content
    front: CardSide,
    /// Back side: Settings/backend
    back: CardSide,
    /// 3D position in workspace
    position: Vector3,
    /// 3D rotation (quaternion)
    rotation: Quaternion,
    /// Scale factor
    scale: f32,
    /// Physical properties
    physics: PhysicsProperties,
    /// Pinned to physical location?
    pinned: Option<PinLocation>,
    /// Associated runtime (VM, native app, etc.)
    runtime: RuntimeHandle,
}

/// Side of a Post Card
pub enum CardSide {
    /// Application window/content
    Application {
        title: String,
        content: CardContent,
        state: AppState,
    },
    /// Settings/backend interface
    Settings {
        controls: Vec<Control>,
        configuration: ConfigMap,
    },
    /// Empty/placeholder
    Empty,
}

/// Content types for Post Cards
pub enum CardContent {
    /// Legacy application running in VM
    LegacyApp(LegacyAppHandle),
    /// Native spatial application
    SpatialApp(SpatialAppHandle),
    /// Web application
    WebApp(WebAppHandle),
    /// Document/viewer
    Document(DocumentHandle),
    /// Media player
    Media(MediaHandle),
    /// Development environment
    Development(DevEnvironmentHandle),
}

/// 3D Rolodex organization system
pub struct Rolodex3D {
    /// Cards arranged in circular formation
    cards: Vec<PostCard>,
    /// Center position of Rolodex
    center: Vector3,
    /// Radius of the circle
    radius: f32,
    /// Angular spacing between cards
    angle_step: f32,
    /// Current rotation angle
    rotation: f32,
    /// Animation state
    animation: AnimationState,
}

/// Physics engine for spatial interactions
pub struct PhysicsEngine {
    /// Gravity vector
    gravity: Vector3,
    /// Air resistance
    drag: f32,
    /// Collision detection
    collisions: CollisionSystem,
    /// Paper airplane physics
    airplane_physics: AirplanePhysics,
}

/// Paper airplane for instant sharing
pub struct PaperAirplane {
    /// Source card ID
    source_id: u64,
    /// Destination (user, wall, etc.)
    destination: AirplaneDestination,
    /// Current position
    position: Vector3,
    /// Velocity vector
    velocity: Vector3,
    /// Rotation
    rotation: Quaternion,
    /// Contained data/work
    payload: AirplanePayload,
    /// Trail effect particles
    trail: Vec<Particle>,
}

impl SpatialWorkspace {
    /// Create a new spatial workspace
    pub fn new() -> Self {
        SpatialWorkspace {
            cards: Mutex::new(Vec::new()),
            rolodex: Rolodex3D::new(Vector3::new(0.0, 1.5, -2.0)),
            physics: PhysicsEngine::new(),
            audio: SpatialAudio::new(),
        }
    }
    
    /// Create a new Post Card
    pub fn create_card(&self, content: CardContent) -> u64 {
        let mut cards = self.cards.lock();
        let id = cards.len() as u64 + 1;
        
        let card = PostCard {
            id,
            front: CardSide::Application {
                title: match &content {
                    CardContent::LegacyApp(_) => "Legacy App".to_string(),
                    CardContent::SpatialApp(_) => "Spatial App".to_string(),
                    CardContent::WebApp(_) => "Web App".to_string(),
                    CardContent::Document(_) => "Document".to_string(),
                    CardContent::Media(_) => "Media".to_string(),
                    CardContent::Development(_) => "Development".to_string(),
                },
                content,
                state: AppState::Running,
            },
            back: CardSide::Settings {
                controls: vec![
                    Control::Slider("Opacity".to_string(), 0.0, 1.0, 1.0),
                    Control::Toggle("Audio".to_string(), true),
                    Control::Button("Close".to_string()),
                ],
                configuration: ConfigMap::new(),
            },
            position: Vector3::zero(),
            rotation: Quaternion::identity(),
            scale: 1.0,
            physics: PhysicsProperties {
                mass: 0.1,
                drag_coefficient: 0.5,
                bounciness: 0.3,
            },
            pinned: None,
            runtime: RuntimeHandle::new(),
        };
        
        cards.push(card);
        
        // Position card in Rolodex
        self.rolodex.add_card(id);
        
        serial_println!("Created Post Card #{}", id);
        id
    }
    
    /// Flip a Post Card to show back side
    pub fn flip_card(&self, card_id: u64) {
        let mut cards = self.cards.lock();
        if let Some(card) = cards.iter_mut().find(|c| c.id == card_id) {
            // Animate flip
            serial_println!("Flipping card #{}", card_id);
            
            // In production: start flip animation
            // For now, just log
        }
    }
    
    /// Pin a card to physical wall/location
    pub fn pin_card(&self, card_id: u64, location: PinLocation) {
        let mut cards = self.cards.lock();
        if let Some(card) = cards.iter_mut().find(|c| c.id == card_id) {
            card.pinned = Some(location);
            
            // Remove from Rolodex
            self.rolodex.remove_card(card_id);
            
            // Position at physical location
            card.position = location.position;
            card.rotation = location.rotation;
            
            serial_println!("Pinned card #{} to {:?}", card_id, location);
        }
    }
    
    /// Create a paper airplane from card content
    pub fn create_paper_airplane(&self, card_id: u64, destination: AirplaneDestination) -> Option<PaperAirplane> {
        let cards = self.cards.lock();
        let card = cards.iter().find(|c| c.id == card_id)?;
        
        // Extract payload from card
        let payload = match &card.front {
            CardSide::Application { content, .. } => {
                match content {
                    CardContent::LegacyApp(handle) => AirplanePayload::AppState(handle.snapshot()),
                    CardContent::SpatialApp(handle) => AirplanePayload::SpatialState(handle.state()),
                    CardContent::WebApp(handle) => AirplanePayload::WebState(handle.url().to_string()),
                    CardContent::Document(handle) => AirplanePayload::Document(handle.content()),
                    CardContent::Media(handle) => AirplanePayload::Media(handle.media_info()),
                    CardContent::Development(handle) => AirplanePayload::Code(handle.code_snapshot()),
                }
            }
            _ => AirplanePayload::Empty,
        };
        
        let airplane = PaperAirplane {
            source_id: card_id,
            destination,
            position: card.position,
            velocity: Vector3::new(0.0, 0.0, -5.0), // Forward velocity
            rotation: card.rotation,
            payload,
            trail: Vec::new(),
        };
        
        serial_println!("Created paper airplane from card #{}", card_id);
        Some(airplane)
    }
    
    /// Flick (throw) a paper airplane
    pub fn flick_airplane(&self, airplane: &mut PaperAirplane, force: Vector3) {
        airplane.velocity = force;
        
        // Start physics simulation
        self.physics.launch_airplane(airplane);
        
        serial_println!("Flicked paper airplane with force {:?}", force);
    }
    
    /// Update workspace physics and animations
    pub fn update(&self, delta_time: f32) {
        // Update Rolodex animation
        self.rolodex.update(delta_time);
        
        // Update physics for all cards
        let mut cards = self.cards.lock();
        for card in cards.iter_mut() {
            if card.pinned.is_none() {
                // Only update physics for unpinned cards
                self.physics.update_card(card, delta_time);
            }
        }
        
        // Update spatial audio
        self.audio.update(&cards);
    }
    
    /// Render the spatial workspace
    pub fn render(&self) {
        let cards = self.cards.lock();
        
        serial_println!("Rendering spatial workspace:");
        serial_println!("  Cards: {}", cards.len());
        serial_println!("  Rolodex position: {:?}", self.rolodex.center);
        
        for card in cards.iter() {
            serial_println!("  Card #{} at {:?} {}", 
                card.id, card.position,
                if card.pinned.is_some() { "(pinned)" } else { "" }
            );
        }
    }
}

impl Rolodex3D {
    fn new(center: Vector3) -> Self {
        Rolodex3D {
            cards: Vec::new(),
            center,
            radius: 2.0,
            angle_step: PI / 6.0, // 30 degrees between cards
            rotation: 0.0,
            animation: AnimationState::Idle,
        }
    }
    
    fn add_card(&mut self, card_id: u64) {
        // Calculate position for new card
        let angle = self.cards.len() as f32 * self.angle_step + self.rotation;
        let position = Vector3::new(
            self.center.x + self.radius * angle.cos(),
            self.center.y,
            self.center.z + self.radius * angle.sin(),
        );
        
        // In production: update actual card position
        serial_println!("Added card #{} to Rolodex at angle {}", card_id, angle);
    }
    
    fn remove_card(&mut self, card_id: u64) {
        // Remove from Rolodex organization
        // Card positions will be recalculated
        serial_println!("Removed card #{} from Rolodex", card_id);
    }
    
    fn update(&mut self, delta_time: f32) {
        // Update rotation animation
        match self.animation {
            AnimationState::Rotating { target, speed } => {
                let delta = target - self.rotation;
                if delta.abs() > 0.01 {
                    self.rotation += delta.signum() * speed * delta_time;
                } else {
                    self.animation = AnimationState::Idle;
                }
            }
            _ => {}
        }
        
        // Recalculate card positions
        // (In production: update actual card transforms)
    }
    
    /// Rotate the Rolodex
    pub fn rotate(&mut self, angle: f32) {
        self.animation = AnimationState::Rotating {
            target: self.rotation + angle,
            speed: 2.0, // radians per second
        };
    }
}

impl PhysicsEngine {
    fn new() -> Self {
        PhysicsEngine {
            gravity: Vector3::new(0.0, -9.8, 0.0),
            drag: 0.1,
            collisions: CollisionSystem::new(),
            airplane_physics: AirplanePhysics::new(),
        }
    }
    
    fn update_card(&self, card: &mut PostCard, delta_time: f32) {
        // Simple physics for demonstration
        // In production: full rigid body physics
        
        // Apply gravity
        // (Simulated - cards float in workspace)
        
        // Dampen any existing velocity
        // card.physics.velocity *= 1.0 - self.drag * delta_time;
    }
    
    fn launch_airplane(&self, airplane: &mut PaperAirplane) {
        // Set up airplane physics
        airplane.velocity.y += 2.0; // Add upward component
        
        // Generate trail particles
        for _ in 0..5 {
            airplane.trail.push(Particle {
                position: airplane.position,
                velocity: Vector3::new(
                    rand::random::<f32>() - 0.5,
                    rand::random::<f32>() - 0.5,
                    rand::random::<f32>() - 0.5,
                ) * 0.5,
                lifetime: 1.0,
            });
        }
    }
}

// Vector and quaternion types for 3D math
#[derive(Debug, Clone, Copy)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vector3 { x, y, z }
    }
    
    pub fn zero() -> Self {
        Vector3 { x: 0.0, y: 0.0, z: 0.0 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quaternion {
    pub fn identity() -> Self {
        Quaternion { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }
    }
}

// Placeholder types for demonstration
pub struct PinLocation {
    pub position: Vector3,
    pub rotation: Quaternion,
    pub wall_id: u32,
}

pub enum AirplaneDestination {
    User(u64),           // User ID
    Wall(u32),           // Wall ID
    Device(String),      // Device name
    Broadcast,           // To everyone
}

pub enum AirplanePayload {
    AppState(serde_json::Value),  // JSON serialized state
    SpatialState(SpatialAppState),
    WebState(String),             // URL
    Document(Vec<u8>),            // Document bytes
    Media(MediaInfo),
    Code(String),                 // Source code
    Empty,
}

pub struct PhysicsProperties {
    pub mass: f32,
    pub drag_coefficient: f32,
    pub bounciness: f32,
}

pub enum AnimationState {
    Idle,
    Rotating { target: f32, speed: f32 },
}

pub enum Control {
    Slider(String, f32, f32, f32), // label, min, max, value
    Toggle(String, bool),
    Button(String),
}

pub struct ConfigMap(std::collections::HashMap<String, String>);
impl ConfigMap {
    fn new() -> Self { ConfigMap(std::collections::HashMap::new()) }
}

pub struct LegacyAppHandle;
pub struct SpatialAppHandle;
pub struct WebAppHandle;
pub struct DocumentHandle;
pub struct MediaHandle;
pub struct DevEnvironmentHandle;
pub struct RuntimeHandle;
pub struct SpatialAppState;
pub struct MediaInfo;
pub struct CollisionSystem;
pub struct AirplanePhysics;
pub struct SpatialAudio;
pub struct Particle;

impl LegacyAppHandle {
    fn snapshot(&self) -> serde_json::Value { serde_json::json!({}) }
}

impl SpatialAppHandle {
    fn state(&self) -> SpatialAppState { SpatialAppState }
}

impl WebAppHandle {
    fn url(&self) -> &str { "" }
}

impl DocumentHandle {
    fn content(&self) -> Vec<u8> { vec![] }
}

impl MediaHandle {
    fn media_info(&self) -> MediaInfo { MediaInfo }
}

impl DevEnvironmentHandle {
    fn code_snapshot(&self) -> String { String::new() }
}

impl RuntimeHandle {
    fn new() -> Self { RuntimeHandle }
}

impl CollisionSystem {
    fn new() -> Self { CollisionSystem }
}

impl AirplanePhysics {
    fn new() -> Self { AirplanePhysics }
}

impl SpatialAudio {
    fn new() -> Self { SpatialAudio }
    fn update(&self, _cards: &[PostCard]) {}
}

// Simple RNG for particle effects
mod rand {
    use spin::Mutex;
    static mut SEED: Mutex<u32> = Mutex::new(12345);
    
    pub fn random<T>() -> f32 
    where
        f32: From<u32>,
    {
        unsafe {
            let mut seed = SEED.lock();
            *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (*seed >> 16) as f32 / 65535.0
        }
    }
}

/// Test the spatial workspace system
pub fn test_spatial_workspace() {
    serial_println!("Testing Post Card Spatial Workspace...");
    
    let workspace = SpatialWorkspace::new();
    
    // Create some test cards
    let card_ids = vec![
        workspace.create_card(CardContent::LegacyApp(LegacyAppHandle)),
        workspace.create_card(CardContent::WebApp(WebAppHandle)),
        workspace.create_card(CardContent::Document(DocumentHandle)),
        workspace.create_card(CardContent::Development(DevEnvironmentHandle)),
    ];
    
    serial_println!("✓ Created {} Post Cards", card_ids.len());
    
    // Test flipping a card
    workspace.flip_card(card_ids[0]);
    
    // Test pinning a card
    let pin_location = PinLocation {
        position: Vector3::new(1.0, 1.5, -1.0),
        rotation: Quaternion::identity(),
        wall_id: 1,
    };
    workspace.pin_card(card_ids[1], pin_location);
    
    // Test creating a paper airplane
    if let Some(mut airplane) = workspace.create_paper_airplane(card_ids[2], AirplaneDestination::User(42)) {
        workspace.flick_airplane(&mut airplane, Vector3::new(1.0, 2.0, -3.0));
        serial_println!("✓ Created and flicked paper airplane");
    }
    
    // Render workspace
    workspace.render();
    
    // Update workspace
    workspace.update(0.016); // ~60 FPS delta
    
    serial_println!("✓ Spatial workspace test complete");
}