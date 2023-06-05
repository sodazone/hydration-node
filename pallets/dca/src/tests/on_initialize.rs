// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::tests::*;
use crate::{
	assert_balance, assert_executed_buy_trades, assert_executed_sell_trades, assert_number_of_executed_buy_trades,
	assert_number_of_executed_sell_trades, assert_scheduled_ids, assert_that_schedule_has_been_removed_from_storages,
	Error, Event as DcaEvent, Order, Permill, ScheduleId,
};
use frame_support::assert_ok;
use frame_support::traits::OnInitialize;
use hydradx_traits::router::PoolType;
use hydradx_traits::router::PoolType::Omnipool;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError;
use std::borrow::Borrow;
use std::ops::RangeInclusive;

#[test]
fn successfull_sell_dca_execution_should_emit_trade_executed_event() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			expect_events(vec![
				DcaEvent::TradeExecuted {
					id: schedule_id,
					who: ALICE,
					amount_in: amount_to_sell,
					amount_out: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 601,
				}
				.into(),
			]);
		});
}

#[test]
fn successfull_buy_dca_execution_should_emit_trade_executed_event() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_buy = 10 * ONE;
			let max_limit = 50 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			expect_events(vec![
				DcaEvent::TradeExecuted {
					id: schedule_id,
					who: ALICE,
					amount_in: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
					amount_out: amount_to_buy,
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 601,
				}
				.into(),
			]);
		});
}

#[test]
fn one_sell_dca_execution_should_unreserve_amount_in() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, initial_alice_hdx_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			let remaining_named_reserve = total_amount - amount_to_sell - SELL_DCA_FEE_IN_NATIVE;
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: amount_to_sell,
				min_buy_amount: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
			}]);

			assert_eq!(remaining_named_reserve, Currencies::reserved_balance(HDX, &ALICE));

			let schedule_id = 0;
			expect_events(vec![
				DcaEvent::TradeExecuted {
					id: schedule_id,
					who: ALICE,
					amount_in: amount_to_sell,
					amount_out: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 601,
				}
				.into(),
			])
		});
}

#[test]
fn sell_schedule_should_sell_remaining_when_there_is_not_enough_left() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, initial_alice_hdx_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = *AMOUNT_OUT_FOR_OMNIPOOL_SELL * 3 / 2;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				min_buy_amount: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
			}]);

			set_to_blocknumber(601);

			//Assert
			assert_executed_sell_trades!(vec![
				SellExecution {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
					min_buy_amount: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				},
				SellExecution {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: total_amount - amount_to_sell - SELL_DCA_FEE_IN_NATIVE - SELL_DCA_FEE_IN_NATIVE,
					min_buy_amount: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				}
			]);

			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			assert_that_dca_is_completed(ALICE, 0);
		});
}

#[test]
fn sell_schedule_should_continue_when_there_is_exact_amount_in_left_as_remaining() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, initial_alice_hdx_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = *AMOUNT_OUT_FOR_OMNIPOOL_SELL * 2 + SELL_DCA_FEE_IN_NATIVE;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(1)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				min_buy_amount: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
			}]);

			assert!(DCA::schedules(0).is_some());
		});
}

#[test]
fn one_buy_dca_execution_should_unreserve_exact_amount_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_buy = 10 * ONE;
			let max_limit = 50 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![BuyExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: amount_to_buy,
				max_sell_amount: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
			}]);

			assert_eq!(
				total_amount - CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY - BUY_DCA_FEE_IN_NATIVE,
				Currencies::reserved_balance(HDX, &ALICE)
			);
		});
}

#[test]
fn one_buy_dca_execution_should_calculate_exact_amount_in_when_multiple_pools_involved() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_buy = 10 * ONE;
			let max_limit = 50 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![
						Trade {
							pool: PoolType::Omnipool,
							asset_in: HDX,
							asset_out: DAI,
						},
						Trade {
							pool: PoolType::XYK,
							asset_in: DAI,
							asset_out: BTC,
						},
					]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![
				BuyExecution {
					asset_in: HDX,
					asset_out: DAI,
					amount_out: XYK_BUY_CALCULATION_RESULT,
					max_sell_amount: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
				},
				BuyExecution {
					asset_in: DAI,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_sell_amount: XYK_BUY_CALCULATION_RESULT,
				}
			]);

			assert_eq!(
				total_amount - CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY - BUY_DCA_FEE_IN_NATIVE,
				Currencies::reserved_balance(HDX, &ALICE)
			);
		});
}

#[test]
fn full_sell_dca_should_be_completed_with_selling_leftover_in_last_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 3 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL + *AMOUNT_OUT_FOR_OMNIPOOL_SELL / 2;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_sell_trades!(4);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_some_successfull_dca_execution_happened_but_less_than_fee_left() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let total_amount = amount_to_sell + SELL_DCA_FEE_IN_NATIVE + SELL_DCA_FEE_IN_NATIVE / 2;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_sell_trades!(1);
			assert_balance!(ALICE, BTC, *AMOUNT_OUT_FOR_OMNIPOOL_SELL);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_buy_dca_should_be_completed_when_some_successfull_dca_execution_happened_but_less_than_fee_left() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount =
				CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY + BUY_DCA_FEE_IN_NATIVE + BUY_DCA_FEE_IN_NATIVE / 2;
			let amount_to_buy = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_buy_trades!(1);
			assert_balance!(ALICE, BTC, CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_sell_dca_should_be_completed_for_multiple_users() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE), (BOB, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 3 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL + *AMOUNT_OUT_FOR_OMNIPOOL_SELL / 2;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule_for_alice = ScheduleBuilder::new()
				.with_owner(ALICE)
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			let schedule_for_bob = ScheduleBuilder::new()
				.with_owner(BOB)
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule_for_alice,
				Option::None
			));
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(BOB),
				schedule_for_bob,
				Option::None
			));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &BOB));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			assert_eq!(0, Currencies::reserved_balance(HDX, &BOB));

			assert_number_of_executed_sell_trades!(8);

			let schedule_id = 0;
			let schedule_id_2 = 1;
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			assert_that_schedule_has_been_removed_from_storages!(BOB, schedule_id_2);
		});
}

#[test]
fn multiple_sell_dca_should_be_completed_for_one_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE), (BOB, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 3 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL + *AMOUNT_OUT_FOR_OMNIPOOL_SELL / 2;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_owner(ALICE)
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount + total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_sell_trades!(8);

			let schedule_id = 0;
			let schedule_id_2 = 1;
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id_2);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_exact_total_amount_specified_for_the_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 3 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL + 3 * SELL_DCA_FEE_IN_NATIVE;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			assert_number_of_executed_sell_trades!(3);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_buy_dca_should_be_completed_when_some_execution_is_successfull_but_not_enough_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE; //We spend 10 in each trade but also the fee is taken, so it won't be enough for 5th trade
			let amount_to_buy = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 2001);

			//Assert
			assert_number_of_executed_buy_trades!(4);
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn one_buy_dca_execution_should_use_default_max_price_diff_for_max_limit_calculation() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_max_price_difference(Permill::from_percent(25))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_buy = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_number_of_executed_buy_trades!(1);
		});
}

#[test]
fn nothing_should_happen_when_no_schedule_in_storage_for_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Act
			proceed_to_blocknumber(1, 500);

			//Assert
			let schedule_id = 1;
			assert!(DCA::schedules(schedule_id).is_none());
		});
}

#[test]
fn schedule_is_planned_for_next_block_when_one_execution_finished() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: 10 * ONE,
					max_limit: 10 * ONE,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_number_of_executed_buy_trades!(1);

			let schedule_id = 0;
			assert_scheduled_ids!(601, vec![schedule_id]);
		});
}

#[test]
fn schedule_is_planned_with_period_when_block_has_already_planned_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let total_amount = 3 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			let schedule_id = 0;
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::Some(601)
			));

			proceed_to_blocknumber(1, 500);
			let schedule_id_2 = 1;

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_scheduled_ids!(601, vec![schedule_id, schedule_id_2]);
		});
}

#[test]
fn buy_dca_schedule_should_be_retried_when_trade_limit_error_happens() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
					max_limit: 5 * ONE,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			set_to_blocknumber(501);

			assert_number_of_executed_buy_trades!(0);

			let schedule_id = 0;

			assert_scheduled_ids!(511, vec![schedule_id]);
			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries);
			expect_dca_events(vec![
				DcaEvent::TradeFailed {
					id: schedule_id,
					who: ALICE,
					error: Error::<Test>::TradeLimitReached.into(),
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 511,
				}
				.into(),
			]);
		});
}

#[test]
fn sell_dca_schedule_should_be_retried_when_trade_limit_error_happens() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
					min_limit: Balance::MAX,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			set_to_blocknumber(501);

			assert_number_of_executed_sell_trades!(0);

			let schedule_id = 0;
			assert_scheduled_ids!(511, vec![schedule_id]);
			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries);
			expect_dca_events(vec![
				DcaEvent::TradeFailed {
					id: schedule_id,
					who: ALICE,
					error: Error::<Test>::TradeLimitReached.into(),
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 511,
				}
				.into(),
			]);
		});
}

#[test]
fn dca_trade_unallocation_should_be_rolled_back_when_trade_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
					max_limit: 5 * ONE,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			let schedule_id = 0;

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), total_amount);
			assert_eq!(DCA::remaining_amounts(schedule_id).unwrap(), total_amount);

			set_to_blocknumber(501);

			assert_number_of_executed_buy_trades!(0);
			assert_scheduled_ids!(511, vec![schedule_id]);

			assert_eq!(
				Currencies::reserved_balance(HDX, &ALICE),
				total_amount - BUY_DCA_FEE_IN_NATIVE
			);
			assert_eq!(
				DCA::remaining_amounts(schedule_id).unwrap(),
				total_amount - BUY_DCA_FEE_IN_NATIVE
			);
		});
}

#[test]
fn dca_schedule_should_terminate_when_error_is_not_configured_to_continue_on() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, FORBIDDEN_ASSET, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: FORBIDDEN_ASSET,
					asset_out: BTC,
					amount_in: ONE,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: FORBIDDEN_ASSET,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;

			assert_number_of_executed_buy_trades!(0);
			assert!(DCA::schedule_ids_per_block(601).is_empty());
			assert_that_dca_is_terminated(ALICE, schedule_id, pallet_omnipool::Error::<Test>::NotAllowed.into());
		});
}

#[test]
fn dca_schedule_should_continue_on_multiple_failures_then_terminated() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
					max_limit: 5 * ONE,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 0;
			set_to_blocknumber(501);
			assert_scheduled_ids!(511, vec![schedule_id]);

			set_to_blocknumber(511);
			assert_scheduled_ids!(531, vec![schedule_id]);

			set_to_blocknumber(531);
			assert_scheduled_ids!(571, vec![schedule_id]);

			set_to_blocknumber(571);
			assert!(DCA::schedules(schedule_id).is_none());
			assert_number_of_executed_buy_trades!(0);
		});
}

#[test]
fn buy_dca_schedule_should_continue_on_slippage_error() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
					max_limit: Balance::MAX,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 0;
			set_to_blocknumber(501);
			assert_scheduled_ids!(511, vec![schedule_id]);
			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries);
		});
}

#[test]
fn sell_dca_schedule_continue_on_slippage_error() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_sell = ONE;

			set_sell_amount_out(ONE / 10);
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 0;
			set_to_blocknumber(501);
			assert_scheduled_ids!(511, vec![schedule_id]);
			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries);
		});
}

#[test]
fn dca_schedule_retry_should_be_reset_when_successfull_trade_after_failed_ones() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 0;
			set_to_blocknumber(501);
			assert_scheduled_ids!(511, vec![schedule_id]);

			set_to_blocknumber(511);
			assert_scheduled_ids!(531, vec![schedule_id]);

			set_max_price_diff(Permill::from_percent(10));

			set_to_blocknumber(531);
			assert_scheduled_ids!(531 + ONE_HUNDRED_BLOCKS, vec![schedule_id]);
			assert_number_of_executed_sell_trades!(1);

			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(0, retries);
		});
}

#[test]
fn execution_fee_should_be_taken_from_user_in_sold_currency_in_case_of_successful_buy_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE), (ALICE, DAI, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let budget = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(budget)
				.with_order(Order::Buy {
					asset_in: DAI,
					asset_out: BTC,
					amount_out: 10 * ONE,
					max_limit: 50 * ONE,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: DAI,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			set_to_blocknumber(501);

			//Assert
			assert_number_of_executed_buy_trades!(1);
			assert_eq!(
				Currencies::reserved_balance(DAI, &ALICE),
				budget - CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY - BUY_DCA_FEE_IN_DAI
			);
			assert_balance!(ALICE, BTC, CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY);

			assert_balance!(TreasuryAccount::get(), DAI, BUY_DCA_FEE_IN_DAI);
		});
}

#[test]
fn execution_fee_should_be_still_taken_from_user_in_sold_currency_in_case_of_failed_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE), (ALICE, DAI, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let budget = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(budget)
				.with_order(Order::Buy {
					asset_in: DAI,
					asset_out: BTC,
					amount_out: CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY,
					max_limit: 5 * ONE,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: DAI,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			assert_balance!(ALICE, BTC, 0);

			set_to_blocknumber(501);

			//Assert
			assert_number_of_executed_buy_trades!(0);
			assert_eq!(Currencies::reserved_balance(DAI, &ALICE), budget - BUY_DCA_FEE_IN_DAI);
			assert_balance!(TreasuryAccount::get(), DAI, BUY_DCA_FEE_IN_DAI);
		});
}

#[test]
fn execution_fee_should_be_taken_from_user_in_sold_currency_in_case_of_successful_sell_trade() {
	let alice_init_native_balance = 5000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, alice_init_native_balance), (ALICE, DAI, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let amount_in = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let budget = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(budget)
				.with_order(Order::Sell {
					asset_in: DAI,
					asset_out: BTC,
					amount_in,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: DAI,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);

			set_to_blocknumber(501);

			//Assert
			assert_eq!(
				Currencies::reserved_balance(DAI, &ALICE),
				budget - amount_in - SELL_DCA_FEE_IN_DAI
			);
			assert_balance!(ALICE, BTC, *AMOUNT_OUT_FOR_OMNIPOOL_SELL);
			assert_balance!(TreasuryAccount::get(), DAI, SELL_DCA_FEE_IN_DAI);
			assert_number_of_executed_sell_trades!(1);
		});
}

#[test]
fn sell_dca_native_execution_fee_should_be_taken_and_sent_to_treasury() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 3 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));
			assert_balance!(TreasuryAccount::get(), HDX, 0);

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_eq!(
				Currencies::reserved_balance(HDX, &ALICE),
				total_amount - *AMOUNT_OUT_FOR_OMNIPOOL_SELL - SELL_DCA_FEE_IN_NATIVE
			);
			assert_balance!(ALICE, BTC, *AMOUNT_OUT_FOR_OMNIPOOL_SELL);
			assert_balance!(TreasuryAccount::get(), HDX, SELL_DCA_FEE_IN_NATIVE);
			assert_number_of_executed_sell_trades!(1);
		});
}

#[test]
fn buy_dca_native_execution_fee_should_be_taken_and_sent_to_treasury() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1500 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let budget = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: 10 * ONE,
					max_limit: 10 * ONE,
					slippage: Some(Permill::from_percent(20)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(budget, Currencies::reserved_balance(HDX, &ALICE));
			assert_balance!(TreasuryAccount::get(), HDX, 0);

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_eq!(
				Currencies::reserved_balance(HDX, &ALICE),
				budget - CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY - BUY_DCA_FEE_IN_NATIVE
			);
			assert_balance!(ALICE, BTC, CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY);
			assert_balance!(TreasuryAccount::get(), HDX, BUY_DCA_FEE_IN_NATIVE);
			assert_number_of_executed_buy_trades!(1);
		});
}

#[test]
fn slippage_limit_should_be_used_for_sell_dca_when_it_is_bigger_than_specified_trade_min_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let sell_amount = ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: sell_amount,
					min_limit: Balance::MIN,
					slippage: Some(Permill::from_percent(1)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: DAI,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			//No trade happens because slippage limit is too small
			assert_number_of_executed_sell_trades!(0);
			let retries = DCA::retries_on_error(0);
			assert_eq!(1, retries);
		});
}

#[test]
fn slippage_limit_should_be_used_for_buy_dca_when_it_is_smaller_than_specified_trade_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let buy_amount = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: DAI,
					amount_out: buy_amount,
					max_limit: Balance::MAX,
					slippage: Some(Permill::from_percent(1)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: DAI,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			//No trade happens because slippage limit is too small
			assert_number_of_executed_buy_trades!(0);
			let retries = DCA::retries_on_error(0);
			assert_eq!(1, retries);
		});
}

#[test]
fn one_sell_dca_execution_should_be_rescheduled_when_price_diff_is_more_than_max_allowed() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, initial_alice_hdx_balance)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(501)));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![]);
			assert_eq!(
				total_amount - SELL_DCA_FEE_IN_NATIVE,
				Currencies::reserved_balance(HDX, &ALICE)
			);

			let schedule_id = 0;
			assert_scheduled_ids!(511, vec![schedule_id]);
			expect_dca_events(vec![
				DcaEvent::TradeFailed {
					id: schedule_id,
					who: ALICE,
					error: Error::<Test>::PriceUnstable.into(),
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 511,
				}
				.into(),
			]);
		});
}

#[test]
fn one_buy_dca_execution_should_be_rescheduled_when_price_diff_is_more_than_max_allowed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_buy = 10 * ONE;
			let max_limit = 20 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![]);
			assert_eq!(
				total_amount - BUY_DCA_FEE_IN_NATIVE,
				Currencies::reserved_balance(HDX, &ALICE)
			);

			let schedule_id = 0;
			assert_scheduled_ids!(511, vec![schedule_id]);
		});
}

#[test]
fn specified_slippage_should_be_used_in_circuit_breaker_price_check() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 50 * ONE;
			let amount_to_buy = 10 * ONE;
			let max_limit = 20 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: Some(Permill::from_percent(9)),
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![]);
			assert_eq!(
				total_amount - BUY_DCA_FEE_IN_NATIVE,
				Currencies::reserved_balance(HDX, &ALICE)
			);

			let schedule_id = 0;
			assert_scheduled_ids!(511, vec![schedule_id]);

			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries);
		});
}

#[test]
fn dca_should_be_terminated_when_dca_cannot_be_planned_due_to_not_free_blocks() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let one_block = 1;

			let schedule_id = 0;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(one_block)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			for _ in RangeInclusive::new(1, 120) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(502)));
			}

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: amount_to_sell,
				min_buy_amount: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
			}]);

			assert_that_dca_is_terminated(ALICE, schedule_id, Error::<Test>::NoFreeBlockFound.into());
		});
}

#[test]
fn dca_should_be_terminated_when_price_change_is_big_but_no_free_blocks_to_replan() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000000 * ONE)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule_id = 0;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(1)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(501)));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			for _ in RangeInclusive::new(1, 120) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(511)));
			}

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![]);
			assert_that_dca_is_terminated(ALICE, schedule_id, Error::<Test>::NoFreeBlockFound.into());
		});
}

#[test]
fn dca_should_be_executed_and_replanned_through_multiple_blocks_when_all_consquent_blocks_are_planned_fully() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 3000 * ONE;
			let amount_to_sell = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(1)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			for _ in RangeInclusive::new(1, 120) {
				assert_ok!(DCA::schedule(
					RuntimeOrigin::signed(ALICE),
					schedule.clone(),
					Option::Some(501)
				));
			}

			let actual_schedule_ids = DCA::schedule_ids_per_block(501);
			assert_eq!(20, actual_schedule_ids.len());

			let actual_schedule_ids = DCA::schedule_ids_per_block(502);
			assert_eq!(20, actual_schedule_ids.len());

			let actual_schedule_ids = DCA::schedule_ids_per_block(504);
			assert_eq!(20, actual_schedule_ids.len());

			let actual_schedule_ids = DCA::schedule_ids_per_block(508);
			assert_eq!(20, actual_schedule_ids.len());

			let actual_schedule_ids = DCA::schedule_ids_per_block(516);
			assert_eq!(20, actual_schedule_ids.len());

			let actual_schedule_ids = DCA::schedule_ids_per_block(532);
			assert_eq!(20, actual_schedule_ids.len());

			//Act
			proceed_to_blocknumber(501, 600);

			//Assert
			assert_number_of_executed_sell_trades!(2000);

			//Assert if none of the schedule is terminated
			for schedule_id in RangeInclusive::new(0, 119) {
				assert!(DCA::schedules(schedule_id).is_some());
			}
		});
}

#[test]
fn dca_sell_schedule_should_be_completed_after_one_trade_when_total_amount_is_equal_to_amount_in_plus_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let amount_in = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let total_amount = amount_in + SELL_DCA_FEE_IN_NATIVE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			assert_number_of_executed_sell_trades!(1);
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn dca_sell_schedule_should_be_terminated_when_schedule_allocation_is_more_than_reserved_funds() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let amount_in = *AMOUNT_OUT_FOR_OMNIPOOL_SELL;
			let total_amount = amount_in + SELL_DCA_FEE_IN_NATIVE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in,
					min_limit: 5 * ONE,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			Currencies::unreserve_named(&NamedReserveId::get(), HDX, &ALICE, ONE / 2);

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			assert_number_of_executed_sell_trades!(0);
			assert_that_dca_is_terminated(ALICE, schedule_id, Error::<Test>::InvalidState.into());
		});
}

#[test]
fn schedules_are_purged_when_the_block_is_over() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);
			assert_number_of_executed_sell_trades!(3);
			set_to_blocknumber(502);

			//Assert
			let scheduled_ids_for_next_block = DCA::schedule_ids_per_block(501);
			assert_eq!(
				scheduled_ids_for_next_block,
				create_bounded_vec_with_schedule_ids(vec![])
			);
		});
}

#[test]
fn successfull_should_be_terminated_when_only_transaction_fee_left() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = ONE + SELL_DCA_FEE_IN_NATIVE + SELL_DCA_FEE_IN_NATIVE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			expect_events(vec![DcaEvent::Completed {
				id: schedule_id,
				who: ALICE,
			}
			.into()]);
		});
}

#[test]
fn execution_is_still_successfull_when_no_parent_hash_present() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			set_parent_hash(None);

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			expect_events(vec![
				DcaEvent::TradeExecuted {
					id: schedule_id,
					who: ALICE,
					amount_in: amount_to_sell,
					amount_out: *AMOUNT_OUT_FOR_OMNIPOOL_SELL,
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 601,
				}
				.into(),
			]);
		});
}

pub fn proceed_to_blocknumber(from: u64, to: u64) {
	for block_number in RangeInclusive::new(from, to) {
		System::set_block_number(block_number);
		DCA::on_initialize(block_number);
	}
}

pub fn set_to_blocknumber(to: u64) {
	System::set_block_number(to);
	DCA::on_initialize(to);
}

fn assert_that_dca_is_completed(owner: AccountId, schedule_id: ScheduleId) {
	assert_that_schedule_has_been_removed_from_storages!(owner, schedule_id);

	expect_events(vec![DcaEvent::Completed {
		id: schedule_id,
		who: owner,
	}
	.into()]);
}

fn assert_that_dca_is_terminated(owner: AccountId, schedule_id: ScheduleId, error: DispatchError) {
	assert_that_schedule_has_been_removed_from_storages!(owner, schedule_id);

	expect_events(vec![DcaEvent::Terminated {
		id: schedule_id,
		who: owner,
		error,
	}
	.into()]);
}