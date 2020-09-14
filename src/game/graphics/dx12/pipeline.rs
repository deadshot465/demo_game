use winapi::um::d3d12::*;
use wio::com::ComPtr;
use winapi::ctypes::c_void;
use winapi::shared::minwindef::UINT;
use crate::game::shared::util::{log_error, get_nullptr};
use crate::game::shared::structs::PushConstant;
use winapi::um::d3dcommon::ID3DBlob;
use winapi::shared::guiddef::REFGUID;
use winapi::Interface;

pub struct Pipeline {
    pub root_signature: ComPtr<ID3D12RootSignature>,
}

impl Pipeline {
    pub unsafe fn new(device: &ComPtr<ID3D12Device2>) -> Self {
        let root_signature = Self::create_root_signature(device);
        Pipeline {
            root_signature,
        }
    }

    pub unsafe fn create_root_signature(device: &ComPtr<ID3D12Device2>) -> ComPtr<ID3D12RootSignature> {
        let mut feature_data = D3D12_FEATURE_DATA_ROOT_SIGNATURE {
            HighestVersion: D3D_ROOT_SIGNATURE_VERSION_1_1
        };
        let res = device.CheckFeatureSupport(
            D3D12_FEATURE_ROOT_SIGNATURE,
            &mut feature_data as *mut _ as *mut c_void,
            std::mem::size_of::<D3D12_FEATURE_DATA_ROOT_SIGNATURE>() as UINT
        );
        log_error(res, "Current device doesn't support required root signature.");

        let flags = D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT |
            D3D12_ROOT_SIGNATURE_FLAG_DENY_DOMAIN_SHADER_ROOT_ACCESS |
            D3D12_ROOT_SIGNATURE_FLAG_DENY_HULL_SHADER_ROOT_ACCESS;

        let mut vs_descriptor_range = vec![];
        vs_descriptor_range.push(D3D12_DESCRIPTOR_RANGE1 {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
            NumDescriptors: 1,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            Flags: 0,
            OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND
        });

        let mut ps_descriptor_range = vec![];
        ps_descriptor_range.push(D3D12_DESCRIPTOR_RANGE1 {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
            NumDescriptors: 1,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            Flags: 0,
            OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND
        });
        
        let root_constant = D3D12_ROOT_CONSTANTS {
            ShaderRegister: 1,
            RegisterSpace: 0,
            Num32BitValues: (std::mem::size_of::<PushConstant>() / 4) as UINT
        };

        let mut root_parameters = vec![];
        let mut parameter_1 = D3D12_ROOT_PARAMETER1 {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            u: Default::default(),
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX
        };
        parameter_1.u.DescriptorTable_mut()
            .NumDescriptorRanges = vs_descriptor_range.len() as UINT;
        parameter_1.u.DescriptorTable_mut()
            .pDescriptorRanges = vs_descriptor_range.as_ptr();
        root_parameters.push(parameter_1);

        let mut parameter_2 = D3D12_ROOT_PARAMETER1 {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            u: Default::default(),
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL
        };
        parameter_2.u.DescriptorTable_mut()
            .pDescriptorRanges = ps_descriptor_range.as_ptr();
        parameter_2.u.DescriptorTable_mut()
            .NumDescriptorRanges = ps_descriptor_range.len() as UINT;
        root_parameters.push(parameter_2);

        let mut parameter_3 = D3D12_ROOT_PARAMETER1 {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            u: Default::default(),
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL
        };
        let constant = parameter_3.u.Constants_mut();
        *constant = root_constant.clone();
        root_parameters.push(parameter_3);

        let static_sampler = D3D12_STATIC_SAMPLER_DESC {
            Filter: D3D12_FILTER_ANISOTROPIC,
            AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 16,
            ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            BorderColor: D3D12_STATIC_BORDER_COLOR_OPAQUE_BLACK,
            MinLOD: 0.0,
            MaxLOD: 0.0,
            ShaderRegister: 2,
            RegisterSpace: 0,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        };

        let mut versioned_desc = D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
            u: Default::default()
        };
        let desc = versioned_desc.u.Desc_1_1_mut();
        desc.Flags = flags;
        desc.NumParameters = root_parameters.len() as UINT;
        desc.pParameters = root_parameters.as_ptr();
        desc.NumStaticSamplers = 1;
        desc.pStaticSamplers = &static_sampler as *const _;

        let mut success_blob = std::mem::zeroed::<ID3DBlob>();
        let mut error_blob = std::mem::zeroed::<ID3DBlob>();
        let mut ref_1 = &mut success_blob as *mut ID3DBlob;
        let mut ref_2 = &mut error_blob as *mut ID3DBlob;
        let mut res = D3D12SerializeVersionedRootSignature(
            &versioned_desc as *const _,
            &mut ref_1 as *mut _,
            &mut ref_2 as *mut _
        );
        log_error(res, "Failed to create root signature blob.");

        let mut ptr = get_nullptr();
        res = device.CreateRootSignature(
            0,
            ref_1
                .as_ref()
                .unwrap()
                .GetBufferPointer(),
            ref_1
                .as_ref()
                .unwrap()
                .GetBufferSize(),
            &ID3D12RootSignature::uuidof() as REFGUID,
            &mut ptr as *mut _
        );
        log_error(res, "Failed to create root signature.");
        log::info!("Successfully created root signature.");
        ComPtr::from_raw(ptr as *mut ID3D12RootSignature)
    }

    pub async unsafe fn create_graphics_pipelines() {

    }

    /*unsafe fn create_graphics_pipeline(device: &ComPtr<ID3D12Device2>,
                                    root_signature: &ComPtr<ID3D12RootSignature>) -> ComPtr<ID3D12PipelineState> {
        let pipeline_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: root_signature.as_raw(),
            VS: Default::default(),
            PS: Default::default(),
            DS: Default::default(),
            HS: Default::default(),
            GS: Default::default(),
            StreamOutput: Default::default(),
            BlendState: Default::default(),
            SampleMask: 0,
            RasterizerState: Default::default(),
            DepthStencilState: Default::default(),
            InputLayout: Default::default(),
            IBStripCutValue: 0,
            PrimitiveTopologyType: 0,
            NumRenderTargets: 0,
            RTVFormats: [],
            DSVFormat: 0,
            SampleDesc: Default::default(),
            NodeMask: 0,
            CachedPSO: Default::default(),
            Flags: 0
        };
    }*/
}