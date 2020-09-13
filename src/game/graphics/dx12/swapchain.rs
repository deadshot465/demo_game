use wio::com::ComPtr;
use winapi::shared::dxgi1_4::IDXGISwapChain3;
use winapi::shared::dxgi1_2::{IDXGIFactory2, DXGI_SWAP_CHAIN_DESC1, DXGI_SCALING_ASPECT_RATIO_STRETCH, DXGI_ALPHA_MODE_UNSPECIFIED, IDXGISwapChain1, DXGI_SCALING_STRETCH};
use winapi::shared::minwindef::{BOOL, UINT, FALSE};
use winapi::shared::dxgiformat::DXGI_FORMAT_B8G8R8A8_UNORM;
use winapi::shared::dxgitype::{DXGI_SAMPLE_DESC, DXGI_USAGE_RENDER_TARGET_OUTPUT};
use winapi::shared::dxgi::{DXGI_SWAP_EFFECT_DISCARD, DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING};
use winapi::um::d3d12::ID3D12CommandQueue;
use winapi::um::unknwnbase::IUnknown;
use winapi::shared::windef::HWND;
use crate::game::shared::util::{get_nullptr, log_error};
use winapi::_core::mem::ManuallyDrop;

pub struct SwapChain {
    pub swap_chain: ComPtr<IDXGISwapChain3>,
    pub buffer_count: UINT,
    pub width: UINT,
    pub height: UINT,
}

impl SwapChain {
    pub unsafe fn new(command_queue: &ComPtr<ID3D12CommandQueue>, factory: *mut IDXGIFactory2,
                      allow_tearing: BOOL, width: UINT, height: UINT, hwnd: HWND) -> Self {
        log::info!("Tearing support: {}", allow_tearing);
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: FALSE,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 3,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_UNSPECIFIED,
            Flags: if allow_tearing != 0 {
                DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING
            } else {
                0
            }
        };

        let mut ptr = get_nullptr() as *mut IDXGISwapChain1;
        let res = factory
            .as_ref()
            .unwrap()
            .CreateSwapChainForHwnd(
                command_queue.as_raw() as *mut IUnknown,
                hwnd, &desc as *const _, std::ptr::null(),
                std::ptr::null_mut(),
                &mut ptr as *mut _
            );
        log_error(res, "Failed to create dxgi swap chain.");
        log::info!("Swap chain successfully created.");
        let _com_ptr = ComPtr::from_raw(ptr);
        SwapChain {
            swap_chain: _com_ptr.cast::<IDXGISwapChain3>().unwrap(),
            buffer_count: 3,
            width,
            height
        }
    }
}