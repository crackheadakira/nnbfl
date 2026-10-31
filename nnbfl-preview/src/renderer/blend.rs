use nnbfl::bflyt::list::{BlendFactor, BlendOp, LogicOp, Material, MaterialBlendMode};
use wgpu::{BlendComponent, BlendFactor as Factor, BlendOperation, BlendState};

pub fn material_blend(mat: &Material) -> Option<BlendState> {
    let Some(mode) = &mat.blend_mode else {
        return Some(BlendState::ALPHA_BLENDING);
    };

    let color = match mode {
        MaterialBlendMode::None => return Some(BlendState::ALPHA_BLENDING),
        MaterialBlendMode::Logic { logic_op } => return logic_blend(*logic_op),
        MaterialBlendMode::Blend {
            blend_op,
            function_source,
            function_destination,
        } => component(*blend_op, *function_source, *function_destination)?,
    };

    let alpha = match &mat.blend_mode_alpha {
        Some(MaterialBlendMode::Blend {
            blend_op,
            function_source,
            function_destination,
        }) => component(*blend_op, *function_source, *function_destination)
            .unwrap_or(BlendComponent::REPLACE),
        _ => BlendComponent::OVER,
    };

    Some(BlendState { color, alpha })
}

fn component(op: BlendOp, src: BlendFactor, dst: BlendFactor) -> Option<BlendComponent> {
    let operation = match op {
        BlendOp::Invalid => return None,
        BlendOp::Add => BlendOperation::Add,
        BlendOp::Subtract => BlendOperation::Subtract,
        BlendOp::ReverseSubtract => BlendOperation::ReverseSubtract,
        BlendOp::SelectMin => BlendOperation::Min,
        BlendOp::SelectMax => BlendOperation::Max,
    };

    if matches!(operation, BlendOperation::Min | BlendOperation::Max) {
        return Some(BlendComponent {
            src_factor: Factor::One,
            dst_factor: Factor::One,
            operation,
        });
    }

    Some(BlendComponent {
        src_factor: factor(src),
        dst_factor: factor(dst),
        operation,
    })
}

fn factor(factor: BlendFactor) -> Factor {
    match factor {
        BlendFactor::V0 => Factor::Zero,
        BlendFactor::V1_0 => Factor::One,
        BlendFactor::DstColor => Factor::Dst,
        BlendFactor::InvDstColor => Factor::OneMinusDst,
        BlendFactor::SrcAlpha => Factor::SrcAlpha,
        BlendFactor::InvSrcAlpha => Factor::OneMinusSrcAlpha,
        BlendFactor::DstAlpha => Factor::DstAlpha,
        BlendFactor::InvDstAlpha => Factor::OneMinusDstAlpha,
        BlendFactor::SrcColor => Factor::Src,
        BlendFactor::InvSrcColor => Factor::OneMinusSrc,
    }
}

fn logic_blend(op: LogicOp) -> Option<BlendState> {
    let keep = |dst_factor| BlendComponent {
        src_factor: Factor::Zero,
        dst_factor,
        operation: BlendOperation::Add,
    };

    match op {
        LogicOp::NoOp => Some(BlendState {
            color: keep(Factor::One),
            alpha: keep(Factor::One),
        }),
        LogicOp::Clear => Some(BlendState {
            color: keep(Factor::Zero),
            alpha: keep(Factor::Zero),
        }),
        _ => None,
    }
}
