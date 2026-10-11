//! Explicit GLSL discard, rather than implementation-dependent SPIR-V OpKill.
use ash::{vk, Instance};
use std::ffi::CStr;
pub(super) fn negotiate(
    instance: &Instance,
    physical: vk::PhysicalDevice,
    extensions: &[vk::ExtensionProperties],
) -> (
    bool,
    vk::PhysicalDeviceShaderTerminateInvocationFeatures<'static>,
) {
    let has=extensions.iter().any(|e|unsafe{CStr::from_ptr(e.extension_name.as_ptr())}==ash::khr::shader_terminate_invocation::NAME);
    let mut features = vk::PhysicalDeviceShaderTerminateInvocationFeatures::default();
    if has {
        let mut query = vk::PhysicalDeviceFeatures2::default().push_next(&mut features);
        unsafe {
            instance.get_physical_device_features2(physical, &mut query);
        }
    }
    let enabled = admit(
        has,
        features.shader_terminate_invocation != 0,
        std::env::var_os("INGETRAZO_DIAGNOSTIC_LEGACY_DISCARD").is_some(),
    );
    (
        enabled,
        vk::PhysicalDeviceShaderTerminateInvocationFeatures::default()
            .shader_terminate_invocation(enabled),
    )
}

fn admit(extension: bool, feature: bool, legacy_diagnostic: bool) -> bool {
    extension && feature && !legacy_diagnostic
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_termination_keeps_legacy_module_without_unenabled_instructions() {
        assert!(admit(true, true, false));
        assert!(!admit(false, true, false));
        assert!(!admit(true, false, false));
        assert!(!admit(true, true, true));
        fn operations(bytes: &[u8]) -> Vec<u16> {
            let words = bytes
                .chunks_exact(4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
                .collect::<Vec<_>>();
            let mut i = 5;
            let mut result = Vec::new();
            while i < words.len() {
                let word = words[i];
                let count = (word >> 16) as usize;
                assert!(count > 0 && i + count <= words.len());
                result.push(word as u16);
                i += count;
            }
            result
        }
        let explicit = operations(include_bytes!("shaders/texture_shadow_terminate.frag.spv"));
        let legacy = operations(include_bytes!("shaders/texture_shadow.frag.spv"));
        assert!(explicit.contains(&4416));
        assert!(!explicit.contains(&252));
        assert!(legacy.contains(&252));
        assert!(!legacy.contains(&4416));
    }
}
