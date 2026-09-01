use crate::app_loader::AppImage;
use crate::error::Result;
use crate::memory::Memory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppIdentity {
    file_size: usize,
    crc32: u32,
    load_base: u32,
    entry_point: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InstructionPatch {
    pub address: u32,
    pub original: u32,
    pub replacement: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameRateEnhancementProfile {
    pub name: &'static str,
    identity: AppIdentity,
    pub cpu_cycles_per_instruction: u64,
    pub patches: &'static [InstructionPatch],
}

const EXTREME_DRIFT_ORIGINAL_TIMESTEP: u32 = 0x3C02_0001;
const EXTREME_DRIFT_HALF_TIMESTEP: u32 = 0x3402_8000;
const EXTREME_DRIFT_ORIGINAL_TIMER_DIVIDER: u32 = 0x2403_0014;
const EXTREME_DRIFT_DOUBLE_TIMER_DIVIDER: u32 = 0x2403_0028;

const EXTREME_DRIFT_TRIAL_PATCHES: &[InstructionPatch] = &[
    InstructionPatch {
        address: 0x80A1_CEC0,
        original: EXTREME_DRIFT_ORIGINAL_TIMESTEP,
        replacement: EXTREME_DRIFT_HALF_TIMESTEP,
    },
    InstructionPatch {
        address: 0x80A1_99C0,
        original: EXTREME_DRIFT_ORIGINAL_TIMER_DIVIDER,
        replacement: EXTREME_DRIFT_DOUBLE_TIMER_DIVIDER,
    },
];

const EXTREME_DRIFT_FULL_PATCHES: &[InstructionPatch] = &[
    InstructionPatch {
        address: 0x80A8_AEB0,
        original: EXTREME_DRIFT_ORIGINAL_TIMESTEP,
        replacement: EXTREME_DRIFT_HALF_TIMESTEP,
    },
    InstructionPatch {
        address: 0x80A8_79B4,
        original: EXTREME_DRIFT_ORIGINAL_TIMER_DIVIDER,
        replacement: EXTREME_DRIFT_DOUBLE_TIMER_DIVIDER,
    },
];

const FRAME_RATE_ENHANCEMENT_PROFILES: &[FrameRateEnhancementProfile] = &[
    FrameRateEnhancementProfile {
        name: "Extreme Drift (trial)",
        identity: AppIdentity {
            file_size: 12_843_417,
            crc32: 0xFA67_BD03,
            load_base: 0x80A0_0000,
            entry_point: 0x80A0_00A0,
        },
        cpu_cycles_per_instruction: 1,
        patches: EXTREME_DRIFT_TRIAL_PATCHES,
    },
    FrameRateEnhancementProfile {
        name: "Extreme Drift (full)",
        identity: AppIdentity {
            file_size: 15_443_327,
            crc32: 0xA2F6_F7BC,
            load_base: 0x80A0_0000,
            entry_point: 0x80AA_07F0,
        },
        cpu_cycles_per_instruction: 1,
        patches: EXTREME_DRIFT_FULL_PATCHES,
    },
];

fn identity(app: &AppImage) -> AppIdentity {
    AppIdentity {
        file_size: app.data.len(),
        crc32: crc32fast::hash(&app.data),
        load_base: app.load_base(),
        entry_point: app.entry_point(),
    }
}

pub(crate) fn frame_rate_enhancement_profile(
    app: &AppImage,
) -> Option<&'static FrameRateEnhancementProfile> {
    let identity = identity(app);
    FRAME_RATE_ENHANCEMENT_PROFILES
        .iter()
        .find(|profile| profile.identity == identity)
}

pub(crate) fn apply_frame_rate_enhancement(
    memory: &mut Memory,
    profile: &FrameRateEnhancementProfile,
    enabled: bool,
) -> Result<()> {
    for patch in profile.patches {
        let current = memory.read_u32(patch.address)?;
        if current != patch.original && current != patch.replacement {
            return Err(format!(
                "{} frame-rate patch mismatch at {:#010x}: expected {:#010x} or {:#010x}, found {:#010x}",
                profile.name, patch.address, patch.original, patch.replacement, current
            )
            .into());
        }
    }

    for patch in profile.patches {
        memory.write_u32(
            patch.address,
            if enabled {
                patch.replacement
            } else {
                patch.original
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_cover_both_verified_extreme_drift_releases() {
        assert_eq!(FRAME_RATE_ENHANCEMENT_PROFILES.len(), 2);
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[0].name,
            "Extreme Drift (trial)"
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[0].identity.file_size,
            12_843_417
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[0].identity.crc32,
            0xFA67_BD03
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[1].name,
            "Extreme Drift (full)"
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[1].identity.file_size,
            15_443_327
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[1].identity.crc32,
            0xA2F6_F7BC
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[0].patches,
            &[
                InstructionPatch {
                    address: 0x80A1_CEC0,
                    original: EXTREME_DRIFT_ORIGINAL_TIMESTEP,
                    replacement: EXTREME_DRIFT_HALF_TIMESTEP,
                },
                InstructionPatch {
                    address: 0x80A1_99C0,
                    original: EXTREME_DRIFT_ORIGINAL_TIMER_DIVIDER,
                    replacement: EXTREME_DRIFT_DOUBLE_TIMER_DIVIDER,
                },
            ]
        );
        assert_eq!(
            FRAME_RATE_ENHANCEMENT_PROFILES[1].patches,
            &[
                InstructionPatch {
                    address: 0x80A8_AEB0,
                    original: EXTREME_DRIFT_ORIGINAL_TIMESTEP,
                    replacement: EXTREME_DRIFT_HALF_TIMESTEP,
                },
                InstructionPatch {
                    address: 0x80A8_79B4,
                    original: EXTREME_DRIFT_ORIGINAL_TIMER_DIVIDER,
                    replacement: EXTREME_DRIFT_DOUBLE_TIMER_DIVIDER,
                },
            ]
        );
    }

    #[test]
    fn patches_are_reversible_and_reject_unexpected_code() {
        for profile in FRAME_RATE_ENHANCEMENT_PROFILES {
            let mut memory = Memory::new();
            for patch in profile.patches {
                memory.write_u32(patch.address, patch.original).unwrap();
            }

            apply_frame_rate_enhancement(&mut memory, profile, true).unwrap();
            for patch in profile.patches {
                assert_eq!(memory.read_u32(patch.address).unwrap(), patch.replacement);
            }

            apply_frame_rate_enhancement(&mut memory, profile, false).unwrap();
            for patch in profile.patches {
                assert_eq!(memory.read_u32(patch.address).unwrap(), patch.original);
                memory.write_u32(patch.address, 0).unwrap();
            }
            assert!(apply_frame_rate_enhancement(&mut memory, profile, true).is_err());
        }
    }
}
