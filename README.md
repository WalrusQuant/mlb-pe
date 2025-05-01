# MLB Pythagorean Expectation model

This project predicts MLB game outcomes for the 2025 season using the Pythagorean Expectation model. It calculates win probabilities, predicted runs for home and away teams, and total runs for games scheduled on a given day. The model leverages team run differentials from the baseballr package and outputs predictions in a formatted table and CSV file.

---

## Pythagorean Expectation

The Pythagorean Expectation, developed by sabermetrician Bill James, estimates a baseball team's expected win percentage based on runs scored and allowed. The formula is:

Win Percentage = (Runs Scored^2) / (Runs Scored^2 + Runs Allowed^2)

James created this formula to quantify how well a team's run differential predicts its win-loss record. In baseball, scoring more runs while allowing fewer is a strong indicator of success. The formula, inspired by the Pythagorean theorem due to its use of squared terms, captures this relationship effectively. It is widely used in baseball analytics because it provides a simple yet powerful way to estimate team performance and predict game outcomes based on offensive and defensive efficiency relative to league averages.



---

## Features

- Fetches 2025 MLB schedule and game data using baseballr.
  
- Computes team statistics (runs scored, allowed, games played).

- Applies Pythagorean Expectation to estimate win probabilities.

- Uses offensive and defensive strengths to predict runs.

- Converts probabilities to American betting odds to show fair odds. 

- Outputs predictions in a formatted table and CSV file (mlb_predictions.csv).



---

## Installation Guide

1. **Install R and RStudio** *(if not installed)*:  
   - [Download R](https://cran.r-project.org/)  
   - [Download RStudio](https://www.rstudio.com/products/rstudio/download/)

2. **Download the Project Files**:  
   - Clone or download this repository to your local machine.

3. **Install Required Packages**:  
   ```markdown
   install.packages(c("tidyverse", "baseballr", "lubridate", "knitr"))


---

## Usage


1. Open the R script (mlb_predictor.R) in an R environment (e.g., RStudio).

2. Run the script to generate predictions for today's games: 
   ```markdown
   source("mlb_predictor.R")

3. View the formatted table in the console and check mlb_predictions.csv for results.

4. To predict games for a future date (e.g., tomorrow), modify the today variable:



---

## Acknowledgments

- Built with [R Shiny](https://shiny.rstudio.com/)  
- MLB data sourced using the [baseballr]([https://hoopr.sportsdataverse.org/](https://billpetti.github.io/baseballr/)) package  







