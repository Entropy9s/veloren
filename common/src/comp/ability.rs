use crate::{
    comp::{Body, CharacterState, EnergySource, Item, Projectile, StateUpdate},
    states::*,
    sys::character_behavior::JoinData,
};
use specs::{Component, FlaggedStorage};
use specs_idvs::IDVStorage;
use std::time::Duration;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum CharacterAbility {
    BasicMelee {
        buildup_duration: Duration,
        recover_duration: Duration,
        base_damage: u32,
        range: f32,
        max_angle: f32,
    },
    BasicRanged {
        recover_duration: Duration,
        projectile: Projectile,
        projectile_body: Body,
    },
    CastFireball {
        recover_duration: Duration,
        projectile: Projectile,
        projectile_body: Body,
    },
    Boost {
        duration: Duration,
        only_up: bool,
    },
    DashMelee {
        buildup_duration: Duration,
        recover_duration: Duration,
        base_damage: u32,
    },
    BasicBlock,
    Roll,
    TimedCombo {
        buildup_duration: Duration,
        recover_duration: Duration,
        base_damage: u32,
    },
    TripleStrike {
        base_damage: u32,
    },
}

impl CharacterAbility {
    /// Attempts to fulfill requirements, mutating `update` (taking energy) if
    /// applicable.
    pub fn requirements_paid(&self, data: &JoinData, update: &mut StateUpdate) -> bool {
        match self {
            CharacterAbility::Roll => {
                data.physics.on_ground
                    && !data.physics.in_fluid
                    && data.body.is_humanoid()
                    && update
                        .energy
                        .try_change_by(-200, EnergySource::Ability)
                        .is_ok()
            },
            CharacterAbility::DashMelee { .. } => {
                !data.physics.in_fluid
                    && update
                        .energy
                        .try_change_by(-300, EnergySource::Ability)
                        .is_ok()
            },
            CharacterAbility::CastFireball { .. } => {
                !data.physics.in_fluid
                    && update
                        .energy
                        .try_change_by(-500, EnergySource::Ability)
                        .is_ok()
            },
            _ => true,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ItemConfig {
    pub item: Item,
    pub ability1: Option<CharacterAbility>,
    pub ability2: Option<CharacterAbility>,
    pub ability3: Option<CharacterAbility>,
    pub block_ability: Option<CharacterAbility>,
    pub dodge_ability: Option<CharacterAbility>,
}

#[derive(Clone, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct Loadout {
    pub active_item: Option<ItemConfig>,
    pub second_item: Option<ItemConfig>,

    pub shoulder: Option<Item>,
    pub chest: Option<Item>,
    pub belt: Option<Item>,
    pub hand: Option<Item>,
    pub pants: Option<Item>,
    pub foot: Option<Item>,
}

impl From<&CharacterAbility> for CharacterState {
    fn from(ability: &CharacterAbility) -> Self {
        match ability {
            CharacterAbility::BasicMelee {
                buildup_duration,
                recover_duration,
                base_damage,
                range,
                max_angle,
            } => CharacterState::BasicMelee(basic_melee::Data {
                exhausted: false,
                buildup_duration: *buildup_duration,
                recover_duration: *recover_duration,
                base_damage: *base_damage,
                range: *range,
                max_angle: *max_angle,
            }),
            CharacterAbility::BasicRanged {
                recover_duration,
                projectile,
                projectile_body,
            } => CharacterState::BasicRanged(basic_ranged::Data {
                exhausted: false,
                prepare_timer: Duration::default(),
                recover_duration: *recover_duration,
                projectile: projectile.clone(),
                projectile_body: *projectile_body,
            }),
            CharacterAbility::CastFireball {
                recover_duration,
                projectile,
                projectile_body,
            } => CharacterState::CastFireball(cast_fireball::Data {
                exhausted: false,
                prepare_timer: Duration::default(),
                recover_duration: *recover_duration,
                projectile: projectile.clone(),
                projectile_body: *projectile_body,
            }),
            CharacterAbility::Boost { duration, only_up } => CharacterState::Boost(boost::Data {
                duration: *duration,
                only_up: *only_up,
            }),
            CharacterAbility::DashMelee {
                buildup_duration,
                recover_duration,
                base_damage,
            } => CharacterState::DashMelee(dash_melee::Data {
                initialize: true,
                exhausted: false,
                buildup_duration: *buildup_duration,
                recover_duration: *recover_duration,
                base_damage: *base_damage,
            }),
            CharacterAbility::BasicBlock => CharacterState::BasicBlock,
            CharacterAbility::Roll => CharacterState::Roll(roll::Data {
                remaining_duration: Duration::from_millis(300),
            }),
            CharacterAbility::TimedCombo {
                buildup_duration,
                recover_duration,
                base_damage,
            } => CharacterState::TimedCombo(timed_combo::Data {
                buildup_duration: *buildup_duration,
                recover_duration: *recover_duration,
                stage: 0,
                stage_exhausted: false,
                stage_time_active: Duration::default(),
                base_damage: *base_damage,
            }),
            CharacterAbility::TripleStrike { base_damage } => {
                CharacterState::TripleStrike(triple_strike::Data {
                    base_damage: *base_damage,
                    stage: 0,
                    stage_exhausted: false,
                    stage_time_active: Duration::default(),
                    should_transition: true,
                    initialized: false,
                })
            },
        }
    }
}

impl Component for Loadout {
    type Storage = FlaggedStorage<Self, IDVStorage<Self>>;
}
