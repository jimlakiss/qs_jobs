Rails.application.routes.draw do
  devise_for :users

  resources :projects, only: [:index, :show]
  resources :contributors

  root "projects#index"
end