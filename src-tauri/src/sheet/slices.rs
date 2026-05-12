#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSlice {
    pub slice_id: String,
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub order_index: i64,
    pub include: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSliceSaveRequest {
    pub sheet_id: String,
    pub slices: Vec<ManualSlice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSliceSaveResult {
    pub saved_count: i64,
    pub warnings: Vec<String>,
}

pub fn validate_manual_slices(slices: &[ManualSlice]) -> AppResult<()> {
    for slice in slices {
        if slice.w <= 0 || slice.h <= 0 {
            return Err(AppError::new(
                "validation",
                "직접 Slice 영역의 너비와 높이는 1px 이상이어야 합니다.",
            ));
        }
    }
    Ok(())
}

pub fn save_manual_slices_future(
    request: ManualSliceSaveRequest,
) -> AppResult<ManualSliceSaveResult> {
    validate_manual_slices(&request.slices)?;
    Err(AppError::new(
        "future_feature",
        "직접 Slice 지정은 설계만 준비되어 있으며 MVP에서는 메뉴로 노출하지 않습니다.",
    ))
}
