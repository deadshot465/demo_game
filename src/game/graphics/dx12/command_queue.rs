use wio::com::ComPtr;
use winapi::um::d3d12::{ID3D12CommandQueue, ID3D12Device2, D3D12_COMMAND_QUEUE_DESC, D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_PRIORITY_NORMAL, ID3D12CommandAllocator, ID3D12CommandList, ID3D12GraphicsCommandList1};
use crate::game::util::{get_nullptr, log_error};
use winapi::shared::minwindef::{INT, UINT};
use winapi::Interface;
use winapi::shared::guiddef::{REFIID, REFGUID};
use winapi::shared::winerror::FAILED;

pub struct CommandQueue {
    pub command_queue: ComPtr<ID3D12CommandQueue>,
    pub command_allocators: Vec<ComPtr<ID3D12CommandAllocator>>,
    pub command_list: ComPtr<ID3D12GraphicsCommandList1>,
}

impl CommandQueue {
    pub unsafe fn new(device: &ComPtr<ID3D12Device2>, buffer_count: UINT) -> Self {
        let command_queue = Self::create_command_queue(device);
        let command_allocators = Self::create_command_allocators(device, buffer_count);
        let command_list = Self::create_command_list(device, &command_allocators[0]);
        CommandQueue {
            command_queue,
            command_allocators,
            command_list
        }
    }

    unsafe fn create_command_queue(device: &ComPtr<ID3D12Device2>) -> ComPtr<ID3D12CommandQueue> {
        let mut ptr = get_nullptr();
        let desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL as INT,
            Flags: 0,
            NodeMask: 0
        };
        let res = device
            .CreateCommandQueue(&desc as *const _, &ID3D12CommandQueue::uuidof() as REFIID, &mut ptr as *mut _);
        log_error(res, "Failed to create command queue.");
        log::info!("Command queue successfully created.");
        ComPtr::from_raw(ptr as *mut ID3D12CommandQueue)
    }

    pub unsafe fn create_command_allocators(device: &ComPtr<ID3D12Device2>, buffer_count: UINT) -> Vec<ComPtr<ID3D12CommandAllocator>> {
        let mut command_allocators = vec![];
        for i in 0..buffer_count {
            let mut ptr = get_nullptr();
            let res = device.CreateCommandAllocator(
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &ID3D12CommandAllocator::uuidof() as REFGUID,
                &mut ptr as *mut _
            );
            if FAILED(res) {
                log::error!("Failed to create command allocator {}.", i);
            }
            log::info!("Successfully created command allocator {}", i);
            command_allocators.push(ComPtr::from_raw(ptr as *mut ID3D12CommandAllocator));
        }
        command_allocators
    }

    pub unsafe fn create_command_list(device: &ComPtr<ID3D12Device2>, allocator: &ComPtr<ID3D12CommandAllocator>) -> ComPtr<ID3D12GraphicsCommandList1> {
        let mut ptr = get_nullptr();
        let mut res = device.CreateCommandList(
            0, D3D12_COMMAND_LIST_TYPE_DIRECT,
            allocator.as_raw(), std::ptr::null_mut(),
            &ID3D12CommandList::uuidof() as *const _,
            &mut ptr as *mut _
        );
        log_error(res, "Failed to create command list.");
        log::info!("Command list successfully created.");
        let _ptr = ComPtr::from_raw(ptr as *mut ID3D12GraphicsCommandList1);
        res = _ptr.Close();
        log_error(res, "Failed to close command list.");
        log::info!("Command list successfully closed.");
        _ptr
    }
}