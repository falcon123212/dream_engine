use ash::{vk, Entry, Instance, Device};
use ash::extensions::ext;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::ffi::{CStr, CString};
use std::mem::ManuallyDrop;
use log::{info, warn, error};

// On importe notre gestionnaire mémoire
use crate::memory::manager::MemoryManager;

pub struct ForgeContext {
    pub entry: Entry,
    pub instance: Instance,
    pub debug_utils: Option<(ext::DebugUtils, vk::DebugUtilsMessengerEXT)>,
    
    pub physical_device: vk::PhysicalDevice,
    pub device: Device,
    pub queue: vk::Queue,
    pub queue_family: u32,

    // 🧠 SAO MEMORY CORE
    // ManuallyDrop est vital : L'allocateur contient une référence au Device.
    // Rust droppe les champs dans l'ordre de déclaration, mais nous voulons
    // être explicites : L'allocateur DOIT mourir avant le Device.
    pub memory: ManuallyDrop<MemoryManager>,
}

impl ForgeContext {
    pub fn init(window: &impl HasRawDisplayHandle, app_name: &str) -> Self {
        unsafe {
            // 1. CHARGEMENT DE LA LIBRAIRIE VULKAN
            let entry = Entry::load().expect("❌ Driver Vulkan introuvable");
            
            // 2. CONFIGURATION DE L'INSTANCE
            // On demande Vulkan 1.3 minimum pour les fonctionnalités modernes
            let app_name_cstr = CString::new(app_name).unwrap();
            let app_info = vk::ApplicationInfo::builder()
                .application_name(&app_name_cstr)
                .api_version(vk::API_VERSION_1_3);

            // Extensions requises par la fenêtre (Winit)
            let mut extension_names = ash_window::enumerate_required_extensions(window.raw_display_handle())
                .expect("❌ Impossible de lister les extensions de fenêtre")
                .to_vec();
            
            // Extension de Debug
            extension_names.push(ext::DebugUtils::name().as_ptr());

            // Couches de validation (Validation Layers)
            let layer_names = [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
            let layer_raw_names: Vec<*const i8> = layer_names.iter().map(|l| l.as_ptr()).collect();

            let instance_create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&extension_names)
                .enabled_layer_names(&layer_raw_names);

            let instance = entry.create_instance(&instance_create_info, None)
                .expect("❌ Echec création Instance Vulkan");

            // 3. DEBUG UTILS SETUP
            let debug_utils = Self::setup_debug(&entry, &instance);
            info!("🟢 [FORGE] Instance Vulkan 1.3 créée.");

            // 4. SÉLECTION DU GPU (PHYSIQUE)
            let (p_device, q_family) = Self::select_gpu(&instance);

            // 5. CRÉATION DU DEVICE LOGIQUE (Le Cerveau)
            // C'est ici qu'on active l'architecture SAO "Bindless".
            
            // A. Features Vulkan 1.3 (Dynamic Rendering & Sync2)
            let mut features13 = vk::PhysicalDeviceVulkan13Features::builder()
                .synchronization2(true)      // Barrières mémoires simplifiées
                .dynamic_rendering(true);    // Plus de RenderPass objects !

            // B. Features Vulkan 1.2 (Le cœur du Bindless)
            // B. Features Vulkan 1.2 (Le cœur du Bindless)
            let mut features12 = vk::PhysicalDeviceVulkan12Features::builder()
                .buffer_device_address(true)             // Pointeurs GPU (u64)
                .descriptor_indexing(true)               // Tableaux de textures
                .runtime_descriptor_array(true)          // Tableaux de taille variable
                .descriptor_binding_partially_bound(true)// Textures nulles acceptées
                .timeline_semaphore(true);               // Synchro CPU/GPU avancée

            // C. Features 64-bit Integers (Standard Vulkan 1.0)
            let features = vk::PhysicalDeviceFeatures::builder()
                .shader_int64(true);

            // Extensions Device
            let device_extensions = [
                CStr::from_bytes_with_nul(b"VK_KHR_swapchain\0").unwrap().as_ptr(),
                // Plus tard, on ajoutera RayTracing ici
            ];

            let queue_infos = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(q_family)
                .queue_priorities(&[1.0])
                .build()];

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_infos)
                .enabled_extension_names(&device_extensions)
                .enabled_features(&features) // <-- Solution standard pour shader_int64
                .push_next(&mut features13)
                .push_next(&mut features12);

            let device = instance.create_device(p_device, &device_create_info, None)
                .expect("❌ Echec création Device (GPU incompatible ?)");

            let queue = device.get_device_queue(q_family, 0);
            info!("🟢 [FORGE] Device Logique Configuré (Bindless + BDA Actifs).");

            // 6. INITIALISATION MÉMOIRE (GPU-ALLOCATOR)
            // On passe le p_device et device pour configurer l'allocateur
            let memory_manager = MemoryManager::new(&instance, &device, p_device);

            Self {
                entry,
                instance,
                debug_utils: Some(debug_utils),
                physical_device: p_device,
                device,
                queue,
                queue_family: q_family,
                memory: ManuallyDrop::new(memory_manager),
            }
        }
    }

    /// Sélectionne le GPU Discret le plus puissant
    unsafe fn select_gpu(instance: &Instance) -> (vk::PhysicalDevice, u32) {
        let pdevices = instance.enumerate_physical_devices().unwrap();
        
        for p in pdevices {
            let props = instance.get_physical_device_properties(p);
            
            // On veut un GPU dédié (AMD/NVIDIA/INTEL ARC)
            if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                let queues = instance.get_physical_device_queue_family_properties(p);
                for (i, q) in queues.iter().enumerate() {
                    // On veut une queue qui fait GRAPHICS et COMPUTE
                    if q.queue_flags.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE) {
                        let name = CStr::from_ptr(props.device_name.as_ptr());
                        info!("🎮 GPU Sélectionné: {:?}", name);
                        return (p, i as u32);
                    }
                }
            }
        }
        panic!("❌ Aucun GPU Dédié (Discrete) compatible Vulkan trouvé !");
    }

    /// Active les logs de validation
    unsafe fn setup_debug(entry: &Entry, instance: &Instance) -> (ext::DebugUtils, vk::DebugUtilsMessengerEXT) {
        let debug_utils = ext::DebugUtils::new(entry, instance);
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR | 
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL | 
                vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION | 
                vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
            )
            .pfn_user_callback(Some(vulkan_debug_callback));
            
        let messenger = debug_utils.create_debug_utils_messenger(&create_info, None).unwrap();
        (debug_utils, messenger)
    }
}

// Nettoyage manuel propre
impl Drop for ForgeContext {
    fn drop(&mut self) {
        unsafe {
            info!("🛑 [FORGE] Arrêt des systèmes...");
            
            // 1. On attend que le GPU ait fini de travailler
            self.device.device_wait_idle().unwrap();

            // 2. Destruction Mémoire (AVANT le Device)
            ManuallyDrop::drop(&mut self.memory);
            info!("🧠 [MEMORY] Allocateur libéré.");

            // 3. Destruction Device
            self.device.destroy_device(None);
            
            // 4. Destruction Debug
            if let Some((utils, messenger)) = self.debug_utils.take() {
                utils.destroy_debug_utils_messenger(messenger, None);
            }

            // 5. Destruction Instance
            self.instance.destroy_instance(None);
            info!("👋 [FORGE] Shutdown complet.");
        }
    }
}

/// Callback pour les erreurs Vulkan
unsafe extern "system" fn vulkan_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let data = *p_callback_data;
    let message = CStr::from_ptr(data.p_message).to_string_lossy();

    if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::ERROR {
        error!("🔴 [VULKAN] {}", message);
    } else if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::WARNING {
        warn!("⚠️ [VULKAN] {}", message);
    }
    vk::FALSE
}