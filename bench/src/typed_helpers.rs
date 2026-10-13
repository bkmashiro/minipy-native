use minipy_native::{buffer::Quote, error::TaskError};

/// A field in the fixed Quote/v1 record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Price,
    Quantity,
}

fn value(row: &Quote, field: Field) -> i64 {
    match field {
        Field::Price => row.unit_price_cents,
        Field::Quantity => row.quantity,
    }
}

/// Sum `left * right` for rows where `gt_field > gt_bound` and
/// `le_field <= le_bound`.
///
/// Rows are processed in input order. The greater-than predicate is evaluated
/// before the less-than-or-equal predicate, and multiplication before addition;
/// all arithmetic is checked and reports `TaskError::Overflow`.
pub fn sum_product_where_gt_le(
    rows: &[Quote],
    gt_field: Field,
    gt_bound: i64,
    le_field: Field,
    le_bound: i64,
    left: Field,
    right: Field,
) -> Result<i64, TaskError> {
    let mut total = 0_i64;
    for row in rows {
        if value(row, gt_field) > gt_bound && value(row, le_field) <= le_bound {
            let product = value(row, left)
                .checked_mul(value(row, right))
                .ok_or(TaskError::Overflow)?;
            total = total.checked_add(product).ok_or(TaskError::Overflow)?;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(price: i64, quantity: i64) -> Quote {
        Quote {
            unit_price_cents: price,
            quantity,
        }
    }

    #[test]
    fn supports_field_permutations_and_boundary_thresholds() {
        let rows = [quote(10, 2), quote(11, 3), quote(8, 5), quote(12, 1)];

        // Price is strictly greater than 10; quantity is at most 3.
        assert_eq!(
            sum_product_where_gt_le(
                &rows,
                Field::Price,
                10,
                Field::Quantity,
                3,
                Field::Price,
                Field::Quantity,
            ),
            Ok(45)
        );
        // Swapping the predicate fields and product fields remains meaningful.
        assert_eq!(
            sum_product_where_gt_le(
                &rows,
                Field::Quantity,
                2,
                Field::Price,
                11,
                Field::Quantity,
                Field::Price,
            ),
            Ok(73)
        );
        // Equality is rejected by `>` and accepted by `<=`.
        assert_eq!(
            sum_product_where_gt_le(
                &rows,
                Field::Quantity,
                1,
                Field::Price,
                11,
                Field::Price,
                Field::Quantity,
            ),
            Ok(93)
        );
    }

    #[test]
    fn empty_and_fully_filtered_inputs_return_zero() {
        assert_eq!(
            sum_product_where_gt_le(
                &[],
                Field::Price,
                0,
                Field::Quantity,
                i64::MAX,
                Field::Price,
                Field::Quantity,
            ),
            Ok(0)
        );
        assert_eq!(
            sum_product_where_gt_le(
                &[quote(2, 3)],
                Field::Price,
                2,
                Field::Quantity,
                i64::MAX,
                Field::Price,
                Field::Quantity,
            ),
            Ok(0)
        );
    }

    #[test]
    fn filtered_overflow_is_not_evaluated() {
        let rows = [quote(i64::MAX, 2), quote(3, 4)];
        // First row passes `gt`, but fails `le`; its product would overflow.
        assert_eq!(
            sum_product_where_gt_le(
                &rows,
                Field::Quantity,
                0,
                Field::Price,
                3,
                Field::Price,
                Field::Quantity,
            ),
            Ok(12)
        );
        // A row rejected by the first predicate also cannot overflow.
        assert_eq!(
            sum_product_where_gt_le(
                &[quote(i64::MAX, 2)],
                Field::Price,
                i64::MAX,
                Field::Quantity,
                i64::MAX,
                Field::Price,
                Field::Quantity,
            ),
            Ok(0)
        );
    }

    #[test]
    fn selected_multiplication_overflow_is_reported() {
        assert_eq!(
            sum_product_where_gt_le(
                &[quote(i64::MAX, 2)],
                Field::Quantity,
                0,
                Field::Price,
                i64::MAX,
                Field::Price,
                Field::Quantity,
            ),
            Err(TaskError::Overflow)
        );
    }

    #[test]
    fn addition_overflow_is_reported_in_row_order() {
        let rows = [quote(i64::MAX, 1), quote(1, 1)];
        assert_eq!(
            sum_product_where_gt_le(
                &rows,
                Field::Quantity,
                0,
                Field::Price,
                i64::MAX,
                Field::Price,
                Field::Quantity,
            ),
            Err(TaskError::Overflow)
        );
    }
}
