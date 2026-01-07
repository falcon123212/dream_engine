use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4};
use std::marker::PhantomData;

/// Position dans l'Univers (Précision Double - 64 bits)
pub type WorldPos = DVec3;

/// Pointeur GPU Bindless (64-bit Address)
/// Représentation transparente d'un u64.
#[repr(transparent)]
#[derive(Debug)] // On garde Debug, mais on retire Copy et Clone du derive
pub struct GpuPtr<T: ?Sized> {
    pub device_address: u64,
    pub _marker: PhantomData<T>,
}

// ✅ 1. Implémentation Manuelle de COPY/CLONE
// Cela permet au pointeur d'être Copiable même si T ne l'est pas !
impl<T: ?Sized> Clone for GpuPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for GpuPtr<T> {}

// ✅ 2. Implémentation Manuelle de POD/ZEROABLE
// On certifie que c'est juste des bytes, sans padding caché.
unsafe impl<T: ?Sized + 'static> Zeroable for GpuPtr<T> {}
unsafe impl<T: ?Sized + 'static> Pod for GpuPtr<T> {}

impl<T: ?Sized> GpuPtr<T> {
    pub fn new(addr: u64) -> Self {
        Self {
            device_address: addr,
            _marker: PhantomData,
        }
    }
}

/// Instance Data (Ce qui est envoyé au GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub model_matrix: Mat4,
    pub inverse_matrix: Mat4,
    pub material_ptr: u64, // On utilise u64 ici pour éviter les soucis de récursion de types dans bytemuck
    pub geometry_ptr: u64,
}