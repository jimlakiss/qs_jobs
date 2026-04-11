Rails.application.routes.draw do
  devise_for :users, skip: [:registrations]

  resources :projects do
    member do
      get :confirm_destroy
    end
  end

  resources :contributors do
    member do
      get :confirm_destroy
    end
  end

  resources :contributor_types do
    member do
      get :confirm_destroy
    end
  end

  root "projects#index"
end
