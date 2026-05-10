#!/usr/bin/env cargo run --bin train_agent
//! Training script for the RL agent
//!
//! Usage: cargo run --bin train_agent -- --episodes 100 --output model.pt

use anyhow::Result;
use clap::Parser;
use neo4j_graph::GraphEmbeddings;
use rl_agent::{PPOConfig, PPOTrainer};
use std::path::Path;

/// Training configuration
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of training episodes
    #[arg(short, long, default_value_t = 50)]
    episodes: usize,

    /// Output path for trained model
    #[arg(short, long, default_value = "trained_model.pt")]
    output: String,

    /// Learning rate
    #[arg(long, default_value_t = 3e-4)]
    learning_rate: f64,

    /// Batch size
    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    /// PPO clip ratio
    #[arg(long, default_value_t = 0.2)]
    clip_ratio: f64,

    /// Discount factor (gamma)
    #[arg(long, default_value_t = 0.99)]
    gamma: f64,

    /// GAE lambda
    #[arg(long, default_value_t = 0.95)]
    lambda: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🤖 DeFi RL Agent Training Script");
    println!("=================================");

    let args = Args::parse();

    // Initialize graph embeddings
    println!("📊 Loading graph embeddings...");
    let graph_embeddings = GraphEmbeddings::new();

    // Configure PPO
    let config = PPOConfig {
        learning_rate: args.learning_rate,
        batch_size: args.batch_size,
        epochs_per_update: 4,
        clip_ratio: args.clip_ratio,
        gamma: args.gamma,
        lambda: args.lambda,
        max_grad_norm: 0.5,
        value_coef: 0.5,
        entropy_coef: 0.01,
    };

    println!("⚙️  PPO Config: {:?}", config);

    // Create trainer
    let mut trainer = PPOTrainer::new(config, graph_embeddings);

    // Run training
    println!("🚀 Starting training for {} episodes...", args.episodes);
    let start_time = std::time::Instant::now();

    let final_stats = trainer.train(args.episodes).await?;

    let training_time = start_time.elapsed();

    // Print final statistics
    println!("\n📈 Final Training Statistics:");
    println!("============================");
    println!("Episodes: {}", final_stats.episode_count);
    println!("Total Steps: {}", final_stats.total_steps);
    println!("Average Reward: {:.4}", final_stats.average_reward);
    println!("Average Sortino Ratio: {:.4}", final_stats.average_sortino);
    println!("Max Drawdown: {:.2}%", final_stats.max_drawdown);
    println!("Policy Loss: {:.6}", final_stats.policy_loss);
    println!("Training Time: {:.2}s", training_time.as_secs_f64());

    // Evaluate against baseline
    println!("\n🔍 Evaluating against baseline...");
    let baseline_stats = rl_agent::baselines::EqualWeightStrategy.evaluate(10).await?;
    println!("Baseline Sortino: {:.4}", baseline_stats.average_sortino);
    println!("Improvement: {:.2}%",
        (final_stats.average_sortino - baseline_stats.average_sortino) / baseline_stats.average_sortino * 100.0);

    // Save model
    println!("\n💾 Saving trained model to {}...", args.output);
    trainer.save_policy(&args.output)?;

    println!("✅ Training complete! Model saved to {}", args.output);

    Ok(())
}