use crate::game::graphics::dx12::{CommandQueue, DescriptorHeap, Pipeline, Resource, SwapChain};
use crate::game::shared::traits::GraphicsBase;
use crate::game::util::{get_nullptr, log_error};
use crate::game::{Camera, ResourceManager};
use parking_lot::RwLock;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::{Arc, Weak};
use winapi::ctypes::c_void;
use winapi::shared::basetsd::SIZE_T;
use winapi::shared::dxgi1_2::IDXGIFactory2;
use winapi::shared::dxgi1_3::{CreateDXGIFactory2, DXGI_CREATE_FACTORY_DEBUG};
use winapi::shared::dxgi1_5::DXGI_FEATURE_PRESENT_ALLOW_TEARING;
use winapi::shared::dxgi1_6::{
    IDXGIAdapter4, IDXGIFactory6, DXGI_ADAPTER_DESC3, DXGI_ADAPTER_FLAG3_SOFTWARE,
    DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
};
use winapi::shared::guiddef::REFIID;
use winapi::shared::minwindef::{BOOL, FALSE, TRUE, UINT};
use winapi::shared::windef::HWND;
use winapi::shared::winerror::{FAILED, SUCCEEDED};
use winapi::um::d3d12::{
    D3D12CreateDevice, D3D12GetDebugInterface, ID3D12Device2, ID3D12GraphicsCommandList,
};
use winapi::um::d3d12sdklayers::*;
use winapi::um::d3dcommon::D3D_FEATURE_LEVEL_12_1;
use winapi::um::unknwnbase::IUnknown;
use winapi::um::winnt::{RtlZeroMemory, LUID};
use winapi::Interface;
use winit::platform::windows::WindowExtWindows;
use wio::com::ComPtr;

type ResourceManagerHandle =
    Weak<RwLock<ResourceManager<Graphics, Resource, ComPtr<ID3D12GraphicsCommandList>, Resource>>>;

#[allow(dead_code)]
pub struct Graphics {
    camera: Rc<RefCell<Camera>>,
    resource_manager: ResourceManagerHandle,
    debug: ComPtr<ID3D12Debug2>,
    dxgi_factory: ComPtr<IDXGIFactory2>,
    dxgi_adapter: ComPtr<IDXGIAdapter4>,
    device: Arc<ComPtr<ID3D12Device2>>,
    info_queue: ComPtr<ID3D12InfoQueue>,
    command_queue: ManuallyDrop<CommandQueue>,
    swap_chain: ManuallyDrop<SwapChain>,
    descriptor_heap: ManuallyDrop<DescriptorHeap>,
    pipeline: ManuallyDrop<Pipeline>,
}

impl Graphics {
    pub unsafe fn new(
        _window: &winit::window::Window,
        camera: Rc<RefCell<Camera>>,
        resource_manager: ResourceManagerHandle,
    ) -> Self {
        let debug = Self::enable_debug();
        let (factory, adapter) = Self::get_adapter();
        let device = Self::create_device(adapter.as_raw());
        let info_queue = Self::create_info_queue(&device);
        let command_queue = CommandQueue::new(&device, 3);
        let mut tearing_support: BOOL = FALSE;
        factory
            .cast::<winapi::shared::dxgi1_5::IDXGIFactory5>()
            .unwrap()
            .CheckFeatureSupport(
                DXGI_FEATURE_PRESENT_ALLOW_TEARING,
                &mut tearing_support as *mut _ as *mut c_void,
                std::mem::size_of::<UINT>() as UINT,
            );
        let swap_chain = SwapChain::new(
            &command_queue.command_queue,
            factory.as_raw(),
            tearing_support,
            _window.inner_size().width,
            _window.inner_size().height,
            _window.hwnd() as HWND,
        );
        let descriptor_heap = DescriptorHeap::new(&device, &swap_chain);
        let pipeline = Pipeline::new(&device);
        Graphics {
            debug,
            camera,
            resource_manager,
            dxgi_factory: factory,
            dxgi_adapter: adapter,
            device: Arc::new(device),
            info_queue,
            command_queue: ManuallyDrop::new(command_queue),
            swap_chain: ManuallyDrop::new(swap_chain),
            descriptor_heap: ManuallyDrop::new(descriptor_heap),
            pipeline: ManuallyDrop::new(pipeline),
        }
    }

    unsafe fn get_adapter() -> (ComPtr<IDXGIFactory2>, ComPtr<IDXGIAdapter4>) {
        let mut dxgi_factory = std::ptr::null_mut() as *mut c_void;
        let mut res = CreateDXGIFactory2(
            DXGI_CREATE_FACTORY_DEBUG,
            &IDXGIFactory2::uuidof() as REFIID,
            &mut dxgi_factory as *mut _,
        );
        if FAILED(res) {
            log::error!("Failed to create dxgi factory.");
        }
        let factory = ComPtr::from_raw(dxgi_factory as *mut IDXGIFactory2);
        let _factory = factory.cast::<IDXGIFactory6>().unwrap();
        let mut adapter = std::ptr::null_mut() as *mut c_void;
        let mut adapter_index: UINT = 0;
        let mut adapter_ptr = std::ptr::null_mut() as *mut c_void;

        let mut dedicated_memory: SIZE_T = 0;
        while SUCCEEDED(res) {
            res = _factory.EnumAdapterByGpuPreference(
                adapter_index,
                DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                &IDXGIAdapter4::uuidof() as REFIID,
                &mut adapter as *mut _,
            );
            if FAILED(res) {
                break;
            }
            let mut desc: DXGI_ADAPTER_DESC3 = DXGI_ADAPTER_DESC3 {
                Description: [0; 128],
                VendorID: 0,
                DeviceID: 0,
                SubSysID: 0,
                Revision: 0,
                DedicatedVideoMemory: 0,
                DedicatedSystemMemory: 0,
                SharedSystemMemory: 0,
                AdapterLuid: LUID {
                    LowPart: 0,
                    HighPart: 0,
                },
                Flags: 0,
                GraphicsPreemptionGranularity: 0,
                ComputePreemptionGranularity: 0,
            };
            RtlZeroMemory(
                &mut desc as *mut _ as *mut c_void,
                std::mem::size_of::<DXGI_ADAPTER_DESC3>(),
            );
            let _res = (adapter as *mut IDXGIAdapter4)
                .as_ref()
                .unwrap()
                .GetDesc3(&mut desc as *mut _);
            if FAILED(_res) {
                log::error!("Failed to get description of the adapter.");
                break;
            } else {
                log::info!("Dedicated video memory: {}", desc.DedicatedVideoMemory);
            }
            let mut device = std::ptr::null_mut() as *mut c_void;
            if desc.DedicatedVideoMemory > dedicated_memory
                && ((desc.Flags & DXGI_ADAPTER_FLAG3_SOFTWARE) == 0)
                && (SUCCEEDED(D3D12CreateDevice(
                    adapter as *mut IUnknown,
                    D3D_FEATURE_LEVEL_12_1,
                    &ID3D12Device2::uuidof() as REFIID,
                    &mut device as *mut _,
                )))
            {
                log::info!("Device successfully created.");
                dedicated_memory = desc.DedicatedVideoMemory;
                adapter_ptr = adapter;
            }
            adapter_index += 1;
        }
        (factory, ComPtr::from_raw(adapter_ptr as *mut IDXGIAdapter4))
    }

    unsafe fn enable_debug() -> ComPtr<ID3D12Debug2> {
        let mut debug = std::ptr::null_mut() as *mut c_void;
        let res = D3D12GetDebugInterface(&ID3D12Debug2::uuidof() as REFIID, &mut debug as *mut _);
        log_error(res, "Failed to get debug interface.");
        let ptr = ComPtr::from_raw(debug as *mut ID3D12Debug2);
        ptr.cast::<ID3D12Debug>().unwrap().EnableDebugLayer();
        log::info!("Debug interface successfully enabled.");
        ptr
    }

    unsafe fn create_info_queue(device: &ComPtr<ID3D12Device2>) -> ComPtr<ID3D12InfoQueue> {
        let info_queue = device.cast::<ID3D12InfoQueue>().unwrap();
        info_queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_CORRUPTION, TRUE);
        info_queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_ERROR, TRUE);
        info_queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_WARNING, TRUE);

        let mut ignore_severity = vec![D3D12_MESSAGE_SEVERITY_INFO];
        let mut ignore_id = vec![
            D3D12_MESSAGE_ID_CLEARRENDERTARGETVIEW_MISMATCHINGCLEARVALUE,
            D3D12_MESSAGE_ID_MAP_INVALID_NULLRANGE,
            D3D12_MESSAGE_ID_UNMAP_INVALID_NULLRANGE,
        ];

        let filter_desc = D3D12_INFO_QUEUE_FILTER_DESC {
            NumCategories: 0,
            pCategoryList: std::ptr::null_mut(),
            NumSeverities: ignore_severity.len() as UINT,
            pSeverityList: ignore_severity.as_mut_ptr(),
            NumIDs: ignore_id.len() as UINT,
            pIDList: ignore_id.as_mut_ptr(),
        };
        let mut filter = D3D12_INFO_QUEUE_FILTER {
            AllowList: D3D12_INFO_QUEUE_FILTER_DESC {
                NumCategories: 0,
                pCategoryList: std::ptr::null_mut(),
                NumSeverities: 0,
                pSeverityList: std::ptr::null_mut(),
                NumIDs: 0,
                pIDList: std::ptr::null_mut(),
            },
            DenyList: filter_desc,
        };
        let res = info_queue.PushStorageFilter(&mut filter as *mut _);
        log_error(res, "Failed to set up info queue.");
        log::info!("Info queue successfully created and set up.");
        info_queue
    }

    unsafe fn create_device(adapter: *mut IDXGIAdapter4) -> ComPtr<ID3D12Device2> {
        let mut device = get_nullptr();
        let res = D3D12CreateDevice(
            adapter as *mut IUnknown,
            D3D_FEATURE_LEVEL_12_1,
            &ID3D12Device2::uuidof() as REFIID,
            &mut device as *mut _,
        );
        log_error(res, "Failed to create D3D12 device.");
        log::info!("D3D12 device successfully created.");
        ComPtr::from_raw(device as *mut ID3D12Device2)
    }
}

impl GraphicsBase<Resource, ComPtr<ID3D12GraphicsCommandList>, Resource> for Graphics {
    fn get_commands(&self) -> &Vec<ComPtr<ID3D12GraphicsCommandList>> {
        unimplemented!()
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.pipeline);
            ManuallyDrop::drop(&mut self.descriptor_heap);
            ManuallyDrop::drop(&mut self.swap_chain);
        }
    }
}
