Rails.application.routes.draw do
  devise_for :users

  resources :projects
  resources :contributors
  resources :contributor_types

  root "projects#index"
end