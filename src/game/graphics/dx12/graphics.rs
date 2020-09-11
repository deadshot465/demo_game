use std::sync::{Arc};
use winapi::shared::dxgi1_6::{IDXGIFactory6, IDXGIAdapter4};
use winapi::shared::dxgi1_3::{CreateDXGIFactory2, DXGI_CREATE_FACTORY_DEBUG};
use winapi::Interface;
use winapi::shared::guiddef::GUID;
use winapi::ctypes::c_void;
use winapi::shared::dxgi1_2::IDXGIFactory2;
use winapi::um::unknwnbase::IUnknown;

pub struct Graphics {
    dxgi_factory: *mut IDXGIFactory6,
}

impl Graphics {
    pub fn new() -> Self {

    }
}
