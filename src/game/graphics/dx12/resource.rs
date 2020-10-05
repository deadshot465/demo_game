use crate::game::shared::traits::Disposable;
use winapi::um::d3d12::*;
use winapi::shared::basetsd::{UINT64, UINT16};
use winapi::shared::minwindef::UINT;
use winapi::shared::dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_UNKNOWN};
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use wio::com::ComPtr;
use crate::game::shared::util::{get_nullptr, log_error};
use winapi::Interface;
use winapi::shared::guiddef::REFGUID;

#[derive(Copy, Clone, Debug)]
pub enum ResourceType {
    Image, Buffer, Intermediate,
}

#[derive(Clone, Debug)]
pub struct Resource {
    pub resource_type: ResourceType,
    pub resource: ComPtr<ID3D12Resource>,
}

impl Resource {
    pub unsafe fn new(device: &ComPtr<ID3D12Device2>, resource_type: ResourceType, width: UINT64, height: UINT, mip_levels: UINT16, format: DXGI_FORMAT, resource_state: D3D12_RESOURCE_STATES, clear_value: *const D3D12_CLEAR_VALUE, resource_flag: D3D12_RESOURCE_FLAGS) -> Self {
        let mut desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: width,
            Height: height,
            DepthOrArraySize: 1,
            MipLevels: mip_levels,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: resource_flag,
        };

        let mut heap_properties = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 1,
            VisibleNodeMask: 1
        };

        let mut ptr = get_nullptr();
        match resource_type {
            ResourceType::Image => {
                let res = device.CreateCommittedResource(
                    &heap_properties as *const _,
                    D3D12_HEAP_FLAG_NONE,
                    &desc as *const _,
                    resource_state,
                    clear_value,
                    &ID3D12Resource::uuidof() as REFGUID,
                    &mut ptr as *mut _
                );
                log_error(res, "Failed to create texture 2D resource.");
                log::info!("Image resource successfully created.");
            },
            ResourceType::Buffer => {
                desc.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
                desc.Height = 1;
                desc.MipLevels = 1;
                desc.Format = DXGI_FORMAT_UNKNOWN;
                desc.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
                let res = device.CreateCommittedResource(
                    &heap_properties as *const _,
                    D3D12_HEAP_FLAG_NONE,
                    &desc as *const _,
                    resource_state,
                    clear_value,
                    &ID3D12Resource::uuidof() as REFGUID,
                    &mut ptr as *mut _
                );
                log_error(res, "Failed to create buffer.");
                log::info!("Buffer resource successfully created.");
            },
            ResourceType::Intermediate => {
                desc.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
                desc.Height = 1;
                desc.MipLevels = 1;
                desc.Format = DXGI_FORMAT_UNKNOWN;
                desc.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
                heap_properties.Type = D3D12_HEAP_TYPE_UPLOAD;
                let res = device.CreateCommittedResource(
                    &heap_properties as *const _,
                    D3D12_HEAP_FLAG_NONE,
                    &desc as *const _,
                    resource_state,
                    clear_value,
                    &ID3D12Resource::uuidof() as REFGUID,
                    &mut ptr as *mut _
                );
                log_error(res, "Failed to create intermediate buffer.");
                log::info!("Intermediate buffer resource successfully created.");
            }
        }
        Resource {
            resource_type,
            resource: ComPtr::from_raw(ptr as *mut ID3D12Resource)
        }
    }
}

impl Drop for Resource {
    fn drop(&mut self) {

    }
}

impl Disposable for Resource {
    fn dispose(&mut self) {
        unimplemented!()
    }

    fn is_disposed(&self) -> bool {
        unimplemented!()
    }

    fn get_name(&self) -> &str {
        unimplemented!()
    }

    fn set_name(&mut self, _name: String) -> &str {
        unimplemented!()
    }
}