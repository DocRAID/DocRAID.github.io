use crate::module::router::Router;
use ratatui::layout::Rect;

//아래 데이터는 근사치 이며 이후 프레임워크 업데이트에 따라 교체되거나 삭제될 코드.
static X_SYNC: f32 = 9.84998; //9.84998
static Y_SYNC: u16 = 22;

pub fn is_rects_hovered(layout: Rect, mouse_pos: (u32, u32)) -> bool {
    let pos_x_min = layout.x as f32 * X_SYNC;
    let pos_x_max = (layout.x + layout.width + 1) as f32 * X_SYNC;
    let pos_y_min = layout.y * Y_SYNC;
    let pos_y_max = layout.y + layout.height * Y_SYNC;

    ((mouse_pos.0 as f32) >= pos_x_min)
        && ((mouse_pos.0 as f32) <= pos_x_max)
        && ((mouse_pos.1 as u16) >= pos_y_min)
        && ((mouse_pos.1 as u16) <= pos_y_max)
}

pub fn is_range_hovered(x: u16, y: u16, width: u16, height: u16, mouse_pos: (u32, u32)) -> bool {
    let pos_x_min = x as f32 * X_SYNC;
    let pos_x_max = (x + width) as f32 * X_SYNC;
    let pos_y_min = y * Y_SYNC;
    let pos_y_max = (y + height) * Y_SYNC;
    ((mouse_pos.0 as f32) >= pos_x_min)
        && ((mouse_pos.0 as f32) <= pos_x_max)
        && ((mouse_pos.1 as u16) >= pos_y_min)
        && ((mouse_pos.1 as u16) <= pos_y_max)
}

pub fn is_points_hovered(
    x_min: u16,
    x_max: u16,
    y_min: u16,
    y_max: u16,
    mouse_pos: (u32, u32),
) -> bool {
    let pos_x_min = x_min as f32 * X_SYNC;
    let pos_x_max = x_max as f32 * X_SYNC;
    let pos_y_min = y_min * Y_SYNC;
    let pos_y_max = y_max * Y_SYNC;

    ((mouse_pos.0 as f32) >= pos_x_min)
        && ((mouse_pos.0 as f32) <= pos_x_max)
        && ((mouse_pos.1 as u16) >= pos_y_min)
        && ((mouse_pos.1 as u16) <= pos_y_max)
}

pub fn calc_header_button_ranges(router: &Router, width: u16) -> Vec<(usize, usize)> {
    // 각 버튼 문자열 길이 + padding(4)
    let text_widths: Vec<usize> = router
        .nav_bar()
        .iter()
        .map(|route| route.to_string().len() + 4)
        .collect();

    // 전체 너비 합
    let total_width: usize = text_widths.iter().sum();

    // 화면 가운데 기준 시작 위치 offset 계산
    let mut offset = (width as usize / 2).saturating_sub(total_width / 2);

    // width/2가 홀수라면 살짝 조정
    // if (width as usize / 2) % 2 == 1 {
    //     offset += 1;
    // }
    offset += 2; // why do I have to add 2 to match it?????? wtf??

    // 각 버튼 범위(range)를 계산
    let mut result = Vec::with_capacity(text_widths.len());
    for &w in &text_widths {
        let start = offset;
        let end = offset + w - 1;
        result.push((start, end));
        offset += w;
    }
    result
}

pub fn calc_topdown_list_button_ranges(
    layout: Rect,
    item_num: usize,
    hight: usize,
) -> Vec<(usize, usize)> {
    let mut result = Vec::new();

    // info!("1. x is {:?}",layout.x);
    // info!("2. y is {:?}",layout.y);
    // info!("3. width is {:?}",layout.width);
    // info!("4. height is {:?}",layout.height);

    for index in 0..item_num {
        let top = layout.y as usize + index + 1; //for margin
        result.push((top, top + hight));
    }

    // info!("{:?}", result);
    result
}
