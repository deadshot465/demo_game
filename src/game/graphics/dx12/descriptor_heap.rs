use winapi::um::d3d12::{D3D12_DESCRIPTOR_HEAP_DESC, D3D12_DESCRIPTOR_HEAP_TYPE_RTV, ID3D12DescriptorHeap, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, ID3D12Device2, D3D12_CPU_DESCRIPTOR_HANDLE, ID3D12Resource, D3D12_DESCRIPTOR_HEAP_TYPE_DSV, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_CLEAR_VALUE, D3D12_CLEAR_VALUE_u, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL};
use wio::com::ComPtr;
use crate::game::graphics::dx12::{SwapChain, Resource, ResourceType};
use crate::game::shared::util::{get_nullptr, log_error};
use winapi::shared::guiddef::{REFGUID, REFIID};
use winapi::Interface;
use winapi::shared::basetsd::{SIZE_T, UINT64};
use winapi::shared::dxgiformat::DXGI_FORMAT_D32_FLOAT;
use winapi::um::winnt::RtlZeroMemory;
use winapi::ctypes::c_void;
use std::mem::ManuallyDrop;

pub struct DescriptorHeap {
    pub rtv_heap: ComPtr<ID3D12DescriptorHeap>,
    pub rtvs: Vec<ComPtr<ID3D12Resource>>,
    pub dsv_heap: ComPtr<ID3D12DescriptorHeap>,
    pub dsv: ManuallyDrop<Resource>,
}

impl DescriptorHeap {
    pub unsafe fn new(device: &ComPtr<ID3D12Device2>, swap_chain: &SwapChain) -> Self {
        let (rtv_heap, rtvs) = Self::create_render_target_view(device, swap_chain);
        let (dsv_heap, dsv) = Self::create_depth_stencil_view(device, swap_chain);
        DescriptorHeap {
            rtv_heap,
            rtvs,
            dsv_heap,
            dsv: ManuallyDrop::new(dsv)
        }
    }

    unsafe fn create_render_target_view(device: &ComPtr<ID3D12Device2>, swap_chain: &SwapChain) -> (ComPtr<ID3D12DescriptorHeap>, Vec<ComPtr<ID3D12Resource>>) {
        let buffer_count = swap_chain.buffer_count;
        let desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            NumDescriptors: buffer_count,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0
        };
        let mut ptr = get_nullptr();
        let res = device
            .CreateDescriptorHeap(
            &desc as *const _,
            &ID3D12DescriptorHeap::uuidof() as REFGUID,
            &mut ptr as *mut _
        );
        log_error(res, "Failed to create RTV descriptor heap.");
        log::info!("RTV descriptor heap successfully created.");
        let mut handle = (ptr as *mut ID3D12DescriptorHeap)
            .as_ref()
            .unwrap()
            .GetCPUDescriptorHandleForHeapStart();
        let mut rtvs = vec![];
        let increment_size = device
            .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);

        for i in 0..buffer_count {
            let mut _ptr = get_nullptr();
            let res = swap_chain.swap_chain
                .GetBuffer(i, &ID3D12Resource::uuidof() as REFIID, &mut _ptr as *mut _);
            log_error(res, "Failed to get back buffer from swap chain.");
            device
                .CreateRenderTargetView(
                _ptr as *mut ID3D12Resource,
                std::ptr::null(),
                handle
            );
            log::info!("Render target view {} successfully created.", i);
            handle.ptr += (increment_size as SIZE_T);
            let comptr = ComPtr::from_raw(_ptr as *mut ID3D12Resource);
            rtvs.push(comptr);
        }
        (ComPtr::from_raw(ptr as *mut ID3D12DescriptorHeap), rtvs)
    }

    unsafe fn create_depth_stencil_view(device: &ComPtr<ID3D12Device2>, swap_chain: &SwapChain) -> (ComPtr<ID3D12DescriptorHeap>, Resource) {
        let desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            NumDescriptors: 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0
        };
        let mut ptr = get_nullptr();
        let mut res = device.CreateDescriptorHeap(
            &desc as *const _,
            &ID3D12DescriptorHeap::uuidof() as REFGUID,
            &mut ptr as *mut _
        );
        log_error(res, "Failed to create dsv descriptor heap.");
        log::info!("DSV descriptor heap successfully created.");
        let handle = (ptr as *mut ID3D12DescriptorHeap)
            .as_ref()
            .unwrap()
            .GetCPUDescriptorHandleForHeapStart();

        let mut clear_value: D3D12_CLEAR_VALUE = Default::default();
        let mut clear_depth = clear_value.u.DepthStencil_mut();
        (*clear_depth).Depth = 1.0;
        (*clear_depth).Stencil = 0;
        clear_value.Format = DXGI_FORMAT_D32_FLOAT;

        let mut resource = Resource::new(
            device, ResourceType::Image,
            swap_chain.width as UINT64,
            swap_chain.height, 0, DXGI_FORMAT_D32_FLOAT,
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
            &clear_value as *const _,
            D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL
        );
        log::info!("Successfully create resource for dsv.");

        device.CreateDepthStencilView(
            resource.resource.as_raw(),
            std::ptr::null(),
            handle
        );

        (ComPtr::from_raw(ptr as *mut ID3D12DescriptorHeap), resource)
    }
}

impl Drop for DescriptorHeap {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.dsv);
        }
    }
}